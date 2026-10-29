//! The audio device side: a cpal stream that pulls from a shared buffer.
//!
//! cpal streams can't move between threads on every platform, so each one
//! lives on its own thread, which also notices when Windows' default
//! output changes (headphones plugged in) and follows it.

use anyhow::{anyhow, bail, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
#[allow(unused_imports)]
use cpal::Sample as _;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Decoded samples waiting for the device, and how many frames it has taken.
#[derive(Default)]
pub struct Ring {
    pub data: VecDeque<f32>,
    pub played: u64,
    /// The last few thousand frames sent to the device, mixed to mono, for
    /// the visualiser. Only filled while someone is looking.
    pub tap: Vec<f32>,
    pub tap_at: usize,
}

/// Frames kept for the visualiser.
pub const TAP_LEN: usize = 2048;

impl Ring {
    pub fn tap_push(&mut self, s: f32) {
        if self.tap.len() < TAP_LEN {
            self.tap.push(s);
        } else {
            self.tap[self.tap_at] = s;
        }
        self.tap_at = (self.tap_at + 1) % TAP_LEN;
    }
    /// The tapped frames, oldest first.
    pub fn tapped(&self) -> Vec<f32> {
        if self.tap.len() < TAP_LEN {
            return self.tap.clone();
        }
        let mut v = Vec::with_capacity(TAP_LEN);
        v.extend_from_slice(&self.tap[self.tap_at..]);
        v.extend_from_slice(&self.tap[..self.tap_at]);
        v
    }
}

pub struct Shared {
    pub ring: Mutex<Ring>,
    pub paused: AtomicBool,
    /// Linear gain, as f32 bits.
    pub volume: AtomicU32,
    /// Fill `Ring::tap`.
    pub tap: AtomicBool,
}

impl Shared {
    pub fn new() -> Arc<Shared> {
        Arc::new(Shared { ring: Mutex::new(Ring::default()), paused: AtomicBool::new(true), volume: AtomicU32::new(1f32.to_bits()), tap: AtomicBool::new(false) })
    }
    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputInfo {
    pub device: String,
    pub rate: u32,
    pub channels: usize,
}

pub enum OutCmd {
    /// Use this device (by name), or the system default with `None`.
    Device(Option<String>),
    Shutdown,
}

/// Names of the output devices on this PC.
pub fn devices() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices().map(|ds| ds.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

fn pick(name: &Option<String>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if let Some(n) = name {
        if let Ok(mut ds) = host.output_devices() {
            if let Some(d) = ds.find(|d| d.name().ok().as_deref() == Some(n.as_str())) {
                return Ok(d);
            }
        }
    }
    host.default_output_device().ok_or_else(|| anyhow!("No audio output found. Plug in speakers or headphones."))
}

fn build(dev: &cpal::Device, shared: &Arc<Shared>, err_tx: Sender<()>) -> Result<(cpal::Stream, OutputInfo)> {
    let sup = dev.default_output_config().map_err(|e| anyhow!("The audio device didn't answer: {e}"))?;
    let config: cpal::StreamConfig = sup.config();
    let info = OutputInfo { device: dev.name().unwrap_or_else(|_| "Audio output".into()), rate: config.sample_rate.0, channels: config.channels as usize };
    let s = shared.clone();
    let stream = match sup.sample_format() {
        cpal::SampleFormat::F32 => stream::<f32>(dev, &config, s, err_tx)?,
        cpal::SampleFormat::I16 => stream::<i16>(dev, &config, s, err_tx)?,
        cpal::SampleFormat::U16 => stream::<u16>(dev, &config, s, err_tx)?,
        cpal::SampleFormat::I32 => stream::<i32>(dev, &config, s, err_tx)?,
        other => bail!("The audio device wants {other:?} samples, which Arcadia doesn't support yet"),
    };
    stream.play().map_err(|e| anyhow!("Couldn't start the audio device: {e}"))?;
    Ok((stream, info))
}

fn stream<T: SizedSample + FromSample<f32>>(dev: &cpal::Device, config: &cpal::StreamConfig, shared: Arc<Shared>, err_tx: Sender<()>) -> Result<cpal::Stream> {
    let ch = config.channels as usize;
    // ~8 ms smoothing on volume and pause, so neither clicks.
    let k = 1.0 - (-1.0 / (config.sample_rate.0 as f32 * 0.008)).exp();
    let mut gain = 0.0f32;
    dev.build_output_stream(
        config,
        move |data: &mut [T], _| {
            let target = if shared.paused.load(Ordering::Relaxed) { 0.0 } else { shared.volume() };
            if target == 0.0 && gain < 1e-4 {
                gain = 0.0;
                data.fill(T::from_sample(0.0f32));
                return;
            }
            let Some(mut ring) = shared.ring.try_lock() else {
                data.fill(T::from_sample(0.0f32));
                return;
            };
            let frames = data.len() / ch;
            let avail = ring.data.len() / ch;
            let n = frames.min(avail);
            let tap = shared.tap.load(Ordering::Relaxed);
            for f in 0..frames {
                gain += (target - gain) * k;
                let mut mono = 0.0;
                for c in 0..ch {
                    let s = if f < n { ring.data.pop_front().unwrap_or(0.0) } else { 0.0 };
                    mono += s;
                    data[f * ch + c] = T::from_sample((s * gain).clamp(-1.0, 1.0));
                }
                if tap && f < n {
                    ring.tap_push(mono / ch as f32);
                }
            }
            ring.played += n as u64;
        },
        move |e| {
            log::warn!("audio stream error: {e}");
            let _ = err_tx.try_send(());
        },
        None,
    )
    .map_err(|e| anyhow!("Couldn't open the audio device: {e}"))
}

/// Start the output thread. `on_change` hears about every (re)opened device.
pub fn spawn(shared: Arc<Shared>, on_change: impl Fn(Result<OutputInfo, String>) + Send + 'static) -> Sender<OutCmd> {
    let (tx, rx) = crossbeam_channel::unbounded::<OutCmd>();
    std::thread::Builder::new()
        .name("arcadia-audio-out".into())
        .spawn(move || run(shared, rx, on_change))
        .expect("spawn audio output thread");
    tx
}

fn run(shared: Arc<Shared>, rx: Receiver<OutCmd>, on_change: impl Fn(Result<OutputInfo, String>)) {
    let (err_tx, err_rx) = crossbeam_channel::bounded::<()>(1);
    let mut wanted: Option<String> = None;
    let mut current: Option<(cpal::Stream, OutputInfo)> = None;
    let open = |wanted: &Option<String>, current: &mut Option<(cpal::Stream, OutputInfo)>| {
        *current = None;
        match pick(wanted).and_then(|d| build(&d, &shared, err_tx.clone())) {
            Ok((s, info)) => {
                on_change(Ok(info.clone()));
                *current = Some((s, info));
            }
            Err(e) => on_change(Err(e.to_string())),
        }
    };
    open(&wanted, &mut current);
    loop {
        crossbeam_channel::select! {
            recv(rx) -> cmd => match cmd {
                Ok(OutCmd::Device(d)) => { wanted = d; open(&wanted, &mut current); }
                Ok(OutCmd::Shutdown) | Err(_) => return,
            },
            recv(err_rx) -> _ => {
                // Unplugged or reset: give the system a moment, then reopen.
                std::thread::sleep(Duration::from_millis(400));
                open(&wanted, &mut current);
            },
            default(Duration::from_secs(2)) => {
                // Following the system default: notice when it changes.
                if wanted.is_none() {
                    let now = cpal::default_host().default_output_device().and_then(|d| d.name().ok());
                    let had = current.as_ref().map(|(_, i)| i.device.clone());
                    if now.is_some() && now != had {
                        open(&wanted, &mut current);
                    }
                }
            }
        }
    }
}
