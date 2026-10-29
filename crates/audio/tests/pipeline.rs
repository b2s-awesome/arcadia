use arcadia_audio::pipeline::{Mark, Pipeline};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A 16-bit PCM WAV whose samples are a known ramp, so joins can be checked.
pub fn wav(dir: &Path, name: &str, rate: u32, ch: u16, frames: u32, base: i16) -> PathBuf {
    let p = dir.join(name);
    let data_len = frames * ch as u32 * 2;
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap();
    f.write_all(&ch.to_le_bytes()).unwrap();
    f.write_all(&rate.to_le_bytes()).unwrap();
    f.write_all(&(rate * ch as u32 * 2).to_le_bytes()).unwrap();
    f.write_all(&(ch * 2).to_le_bytes()).unwrap();
    f.write_all(&16u16.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&data_len.to_le_bytes()).unwrap();
    for i in 0..frames {
        let v = base.wrapping_add((i % 1000) as i16);
        for _ in 0..ch {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
    }
    p
}

fn drain(p: &mut Pipeline) -> (Vec<f32>, Vec<(usize, Mark)>) {
    let mut out = Vec::new();
    let mut marks = Vec::new();
    for _ in 0..10_000 {
        let before = out.len() / p.out_ch;
        let mut m = Vec::new();
        p.fill(4096, &mut out, &mut m);
        for mk in m {
            let at = match &mk {
                Mark::Start { at, .. } | Mark::End { at } => before + at,
                Mark::Failed { .. } => before,
            };
            let end = matches!(mk, Mark::End { .. });
            marks.push((at, mk));
            if end {
                return (out, marks);
            }
        }
        if !p.is_active() {
            break;
        }
    }
    (out, marks)
}

#[test]
fn gapless_join_is_sample_exact() {
    let t = tempfile::tempdir().unwrap();
    let a = wav(t.path(), "a.wav", 44100, 2, 44100, 100);
    let b = wav(t.path(), "b.wav", 44100, 2, 22050, 5000);
    let mut p = Pipeline::new(44100, 2);
    p.load(1, &a, 0.0).unwrap();
    p.set_next(Some((2, b)));
    let (out, marks) = drain(&mut p);
    assert_eq!(out.len() / 2, 44100 + 22050, "no frames lost or added at the join");
    let start_b = marks.iter().find_map(|(at, m)| matches!(m, Mark::Start { token: 2, .. }).then_some(*at)).unwrap();
    assert_eq!(start_b, 44100);
    // Last frame of A, then first frame of B, back to back.
    let last_a = out[(44100 - 1) * 2];
    let first_b = out[44100 * 2];
    assert!((last_a - (100 + 44099 % 1000) as f32 / 32768.0).abs() < 1e-6);
    assert!((first_b - 5000.0 / 32768.0).abs() < 1e-6);
    assert!(matches!(marks.last().unwrap().1, Mark::End { .. }));
}

#[test]
fn mono_is_duplicated_to_stereo() {
    let t = tempfile::tempdir().unwrap();
    let a = wav(t.path(), "m.wav", 44100, 1, 1000, 1234);
    let mut p = Pipeline::new(44100, 2);
    p.load(1, &a, 0.0).unwrap();
    let (out, _) = drain(&mut p);
    assert_eq!(out.len(), 2000);
    assert_eq!(out[0], out[1]);
}

#[test]
fn resampling_keeps_length() {
    let t = tempfile::tempdir().unwrap();
    let a = wav(t.path(), "hi.wav", 48000, 2, 48000, 0);
    let b = wav(t.path(), "lo.wav", 44100, 2, 44100, 0);
    let mut p = Pipeline::new(44100, 2);
    p.load(1, &a, 0.0).unwrap();
    p.set_next(Some((2, b)));
    let (out, marks) = drain(&mut p);
    let start_b = marks.iter().find_map(|(at, m)| matches!(m, Mark::Start { token: 2, .. }).then_some(*at)).unwrap();
    assert!((start_b as i64 - 44100).abs() <= 1, "48 kHz second became {start_b} frames");
    assert_eq!(out.len() / 2 - start_b, 44100);
}

#[test]
fn seek_lands_exactly() {
    let t = tempfile::tempdir().unwrap();
    let a = wav(t.path(), "s.wav", 44100, 2, 44100, 0);
    let mut p = Pipeline::new(44100, 2);
    p.load(1, &a, 0.0).unwrap();
    let m = p.seek(0.5).unwrap().unwrap();
    assert!(matches!(m, Mark::Start { pos, .. } if (pos - 0.5).abs() < 1e-3));
    let (out, _) = drain(&mut p);
    assert_eq!(out.len() / 2, 22050);
}

#[test]
fn broken_next_track_is_reported_and_skipped() {
    let t = tempfile::tempdir().unwrap();
    let a = wav(t.path(), "a.wav", 44100, 2, 1000, 0);
    let bad = t.path().join("missing.flac");
    let mut p = Pipeline::new(44100, 2);
    p.load(1, &a, 0.0).unwrap();
    p.set_next(Some((2, bad)));
    let (_, marks) = drain(&mut p);
    assert!(marks.iter().any(|(_, m)| matches!(m, Mark::Failed { token: 2, .. })));
    assert!(matches!(marks.last().unwrap().1, Mark::End { .. }));
}

#[test]
fn not_audio_is_a_clear_error() {
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("notes.mp3");
    std::fs::write(&f, b"definitely not audio").unwrap();
    let mut p = Pipeline::new(44100, 2);
    let e = p.load(1, &f, 0.0).unwrap_err().to_string();
    assert!(e.contains("notes.mp3"), "{e}");
}

mod engine {
    use super::wav;
    use arcadia_audio::{Engine, Event};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    fn wait_for(log: &Arc<Mutex<Vec<Event>>>, pred: impl Fn(&Event) -> bool) -> bool {
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            if log.lock().unwrap().iter().any(&pred) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[test]
    fn plays_through_a_queue_and_reports_it() {
        let t = tempfile::tempdir().unwrap();
        let a = wav(t.path(), "a.wav", 44100, 2, 22050, 0);
        let b = wav(t.path(), "b.wav", 44100, 2, 22050, 0);
        let log = Arc::new(Mutex::new(Vec::new()));
        let l2 = log.clone();
        let eng = Engine::start_headless(move |e| l2.lock().unwrap().push(e), 44100, 1.0);
        eng.load(10, a, 0.0, false);
        assert!(wait_for(&log, |e| matches!(e, Event::Track { token: 10, .. })));
        eng.set_next(Some((11, b)));
        assert!(wait_for(&log, |e| matches!(e, Event::Track { token: 11, .. })), "gapless advance reported");
        assert!(wait_for(&log, |e| matches!(e, Event::Ended)), "end of queue reported");
        let l = log.lock().unwrap();
        let t10 = l.iter().position(|e| matches!(e, Event::Track { token: 10, .. })).unwrap();
        let t11 = l.iter().position(|e| matches!(e, Event::Track { token: 11, .. })).unwrap();
        let end = l.iter().position(|e| matches!(e, Event::Ended)).unwrap();
        assert!(t10 < t11 && t11 < end);
        assert!(l.iter().any(|e| matches!(e, Event::Position { token: 10, .. })));
    }

    #[test]
    fn pause_holds_position_and_seek_moves_it() {
        let t = tempfile::tempdir().unwrap();
        let a = wav(t.path(), "a.wav", 44100, 2, 44100 * 4, 0);
        let log = Arc::new(Mutex::new(Vec::new()));
        let l2 = log.clone();
        let eng = Engine::start_headless(move |e| l2.lock().unwrap().push(e), 44100, 1.0);
        eng.load(1, a, 0.0, true);
        assert!(wait_for(&log, |e| matches!(e, Event::Track { token: 1, .. })));
        std::thread::sleep(Duration::from_millis(300));
        assert!(!log.lock().unwrap().iter().any(|e| matches!(e, Event::Position { pos, .. } if *pos > 0.05)), "paused means no progress");
        eng.seek(3.0);
        assert!(wait_for(&log, |e| matches!(e, Event::Position { pos, .. } if (*pos - 3.0).abs() < 0.01)));
        eng.pause(false);
        assert!(wait_for(&log, |e| matches!(e, Event::Ended)));
    }

    #[test]
    fn a_late_next_track_still_follows_on() {
        let t = tempfile::tempdir().unwrap();
        let a = wav(t.path(), "a.wav", 44100, 2, 4410, 0);
        let b = wav(t.path(), "b.wav", 44100, 2, 44100, 0);
        let log = Arc::new(Mutex::new(Vec::new()));
        let l2 = log.clone();
        let eng = Engine::start_headless(move |e| l2.lock().unwrap().push(e), 44100, 1.0);
        eng.load(1, a, 0.0, false);
        assert!(wait_for(&log, |e| matches!(e, Event::Track { token: 1, .. })));
        // A is fully decoded already; B arrives while A is still audible.
        std::thread::sleep(Duration::from_millis(30));
        eng.set_next(Some((2, b)));
        assert!(wait_for(&log, |e| matches!(e, Event::Track { token: 2, .. })));
        assert!(wait_for(&log, |e| matches!(e, Event::Ended)));
        assert_eq!(log.lock().unwrap().iter().filter(|e| matches!(e, Event::Ended)).count(), 1);
    }
}
