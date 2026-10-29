//! One audio file, decoded to interleaved `f32` frames.

use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// File extensions the engine can decode.
pub const EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "wave", "ogg", "oga", "m4a", "mp4", "m4b", "aac", "adts"];

pub struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    pub rate: u32,
    pub channels: usize,
    /// Length in seconds, when the file says.
    pub duration: Option<f64>,
    /// Decoded but not yet handed out, interleaved.
    pending: Vec<f32>,
    /// Frames to drop after an accurate seek lands early.
    skip: u64,
    scratch: Option<SampleBuffer<f32>>,
    eof: bool,
}

impl Source {
    pub fn open(path: &Path) -> Result<Source> {
        let file = std::fs::File::open(path).with_context(|| format!("Couldn't open {}", path.display()))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(&ext.to_ascii_lowercase());
        }
        // Gapless: trims encoder delay and padding (MP3 LAME headers, AAC iTunSMPB).
        let fmt_opts = FormatOptions { enable_gapless: true, ..Default::default() };
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &MetadataOptions::default())
            .map_err(|e| anyhow!("{} isn't a format Arcadia can play ({e})", file_name(path)))?;
        let format = probed.format;
        let track = format
            .default_track()
            .filter(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .or_else(|| format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL))
            .ok_or_else(|| anyhow!("{} has no audio in it", file_name(path)))?;
        let track_id = track.id;
        let params = track.codec_params.clone();
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| anyhow!("{} uses a codec Arcadia can't decode yet ({e})", file_name(path)))?;
        let duration = match (params.n_frames, params.sample_rate) {
            (Some(n), Some(r)) if r > 0 => Some(n as f64 / r as f64),
            _ => None,
        };
        let mut s = Source {
            format,
            decoder,
            track_id,
            rate: params.sample_rate.unwrap_or(0),
            channels: params.channels.map(|c| c.count()).unwrap_or(0),
            duration,
            pending: Vec::new(),
            skip: 0,
            scratch: None,
            eof: false,
        };
        // Decode the first packet now: it settles the real rate and channel
        // count (some files only say once decoding starts) and surfaces a
        // broken file before it's queued.
        let first = s.decode_packet()?;
        if s.rate == 0 || s.channels == 0 {
            bail!("{} doesn't say its sample rate", file_name(path));
        }
        if let Some(f) = first {
            s.pending = f;
        }
        Ok(s)
    }

    /// Next block of interleaved samples (in this file's own channel
    /// layout), or `None` at the end.
    pub fn read(&mut self) -> Result<Option<Vec<f32>>> {
        if !self.pending.is_empty() {
            let mut v = std::mem::take(&mut self.pending);
            self.apply_skip(&mut v);
            if !v.is_empty() {
                return Ok(Some(v));
            }
        }
        loop {
            match self.decode_packet()? {
                None => return Ok(None),
                Some(mut v) => {
                    self.apply_skip(&mut v);
                    if !v.is_empty() {
                        return Ok(Some(v));
                    }
                }
            }
        }
    }

    fn apply_skip(&mut self, v: &mut Vec<f32>) {
        if self.skip == 0 {
            return;
        }
        let frames = (v.len() / self.channels) as u64;
        let drop = self.skip.min(frames);
        v.drain(..(drop as usize * self.channels));
        self.skip -= drop;
    }

    fn decode_packet(&mut self) -> Result<Option<Vec<f32>>> {
        if self.eof {
            return Ok(None);
        }
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.eof = true;
                    return Ok(None);
                }
                Err(SymError::ResetRequired) => {
                    // A chained stream changed parameters; carry on with a fresh decoder.
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(anyhow!("Couldn't read the file: {e}")),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                // A damaged frame: skip it, as every player does.
                Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
                Err(e) => return Err(anyhow!("Couldn't decode the file: {e}")),
            };
            let spec = *decoded.spec();
            if decoded.frames() == 0 {
                continue;
            }
            if self.rate == 0 {
                self.rate = spec.rate;
            }
            if self.channels == 0 {
                self.channels = spec.channels.count();
            }
            let need = decoded.capacity() as u64;
            let sb = match &mut self.scratch {
                Some(sb) if sb.capacity() as u64 >= need => sb,
                slot => slot.insert(SampleBuffer::<f32>::new(need, spec)),
            };
            sb.copy_interleaved_ref(decoded);
            return Ok(Some(sb.samples().to_vec()));
        }
    }

    /// Seek to `secs`. Returns where playback will actually resume.
    pub fn seek(&mut self, secs: f64) -> Result<f64> {
        let secs = secs.max(0.0);
        let time = Time::new(secs.trunc() as u64, secs.fract());
        let to = self
            .format
            .seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(self.track_id) })
            .map_err(|e| anyhow!("Couldn't seek: {e}"))?;
        self.decoder.reset();
        self.pending.clear();
        self.eof = false;
        // The reader lands on a packet boundary at or before the target;
        // drop the frames in between so the position is exact.
        self.skip = to.required_ts.saturating_sub(to.actual_ts);
        let tb = self.format.tracks().iter().find(|t| t.id == self.track_id).and_then(|t| t.codec_params.time_base);
        Ok(match tb {
            Some(tb) => {
                let t = tb.calc_time(to.required_ts);
                t.seconds as f64 + t.frac
            }
            None => secs,
        })
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string())
}

/// Map interleaved samples from `from` channels to `to` channels. Stereo
/// and mono are exact; surround is folded down to stereo with the usual
/// −3 dB centre and surrounds.
pub fn remap(input: &[f32], from: usize, to: usize, out: &mut Vec<f32>) {
    let frames = input.len() / from.max(1);
    out.reserve(frames * to);
    for f in 0..frames {
        let s = &input[f * from..f * from + from];
        let (l, r) = match from {
            1 => (s[0], s[0]),
            2 => (s[0], s[1]),
            n => {
                // FL FR FC LFE BL BR (SL SR): the common WAV/FLAC order.
                const K: f32 = std::f32::consts::FRAC_1_SQRT_2;
                let c = if n > 2 { s[2] } else { 0.0 };
                let bl = if n > 4 { s[4] } else { 0.0 };
                let br = if n > 5 { s[5] } else { 0.0 };
                let sl = if n > 6 { s[6] } else { 0.0 };
                let sr = if n > 7 { s[7] } else { 0.0 };
                let norm = 1.0 / (1.0 + K * 2.0);
                ((s[0] + K * c + K * (bl + sl)) * norm, (s[1] + K * c + K * (br + sr)) * norm)
            }
        };
        match to {
            1 => out.push((l + r) * 0.5),
            _ => {
                out.push(l);
                out.push(r);
                out.extend(std::iter::repeat_n(0.0, to - 2));
            }
        }
    }
}
