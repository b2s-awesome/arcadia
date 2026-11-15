//! Frame-time statistics.
//!
//! One documented method, used everywhere (live monitor, benchmarks, reports):
//!
//! * **Average FPS** = frames ÷ elapsed time = `1000 · n / Σ frame_time_ms`.
//!   It is *not* the mean of per-frame FPS values, which over-weights fast frames.
//! * **Frame-time percentiles** (p50, p90, p95, p99, p99.9) use linear
//!   interpolation between closest ranks on the sorted frame times (the same
//!   rule as NumPy's default `percentile`).
//! * **1% low / 0.1% low FPS** = average FPS over the slowest 1% / 0.1% of
//!   frames: sort frame times descending, take the first `ceil(n · p)` frames,
//!   and compute `1000 · k / Σ` over that bucket.
//!   A low is only reported when the bucket holds at least
//!   [`MIN_BUCKET_FRAMES`] frames (300 frames for the 1% low, 3000 for the
//!   0.1% low). Below that the value is `None` and never estimated.
//! * **Percentile FPS** (`p1_fps`, `p01_fps`) = `1000 / p99` and `1000 / p99.9`
//!   frame time. Reported next to the lows because some tools use that
//!   definition; the two are labelled separately and never mixed.
//! * **Consistency** = `p50 / p99` frame time, as a percentage. 100% means the
//!   slowest 1% of frames take no longer than a typical frame.
//! * **Stutter frames** = frames longer than both 2× the median frame time and
//!   the median + 4 ms.

use serde::{Deserialize, Serialize};

/// Fewest frames a "low" bucket may hold before the value is reported.
pub const MIN_BUCKET_FRAMES: usize = 3;
/// Fewest frames before any statistic is reported.
pub const MIN_FRAMES: usize = 10;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameStats {
    pub frames: usize,
    pub duration_ms: f64,
    pub avg_fps: Option<f64>,
    pub avg_frame_time_ms: Option<f64>,
    pub min_frame_time_ms: Option<f64>,
    pub max_frame_time_ms: Option<f64>,
    pub p50_ms: Option<f64>,
    pub p90_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub p999_ms: Option<f64>,
    /// Average FPS over the slowest 1% of frames.
    pub low_1pct_fps: Option<f64>,
    /// Average FPS over the slowest 0.1% of frames.
    pub low_01pct_fps: Option<f64>,
    /// 1000 / p99 frame time.
    pub p1_fps: Option<f64>,
    /// 1000 / p99.9 frame time.
    pub p01_fps: Option<f64>,
    pub stddev_ms: Option<f64>,
    /// p50 / p99 frame time, in percent.
    pub consistency_pct: Option<f64>,
    pub stutter_frames: Option<usize>,
    /// Why some values are missing, in plain words.
    pub notes: Vec<String>,
}

/// Linear-interpolated percentile of an ascending slice. `p` is 0..=100.
pub fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    if sorted.len() == 1 {
        return Some(sorted[0]);
    }
    let rank = (p.clamp(0.0, 100.0) / 100.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    Some(sorted[lo] + (sorted[hi] - sorted[lo]) * frac)
}

/// Average FPS over the slowest `per_mille`/1000 of frames (10 = the 1% low,
/// 1 = the 0.1% low). `sorted` must be ascending. `None` when the bucket
/// would hold fewer than [`MIN_BUCKET_FRAMES`] frames.
pub fn low_fps(sorted: &[f64], per_mille: usize) -> Option<f64> {
    let n = sorted.len();
    // Integer arithmetic: no floating-point fuzz at the thresholds.
    let k = n * per_mille / 1000;
    if k < MIN_BUCKET_FRAMES {
        return None;
    }
    let sum: f64 = sorted[n - k..].iter().sum();
    (sum > 0.0).then(|| 1000.0 * k as f64 / sum)
}

/// Frames needed before a low with this share can be reported.
pub fn frames_needed(per_mille: usize) -> usize {
    (MIN_BUCKET_FRAMES * 1000).div_ceil(per_mille.max(1))
}

/// Compute statistics from frame times in milliseconds. Non-finite and
/// non-positive samples are dropped (and counted in `notes`).
pub fn compute(frame_times_ms: &[f64]) -> FrameStats {
    let mut v: Vec<f64> = frame_times_ms.iter().copied().filter(|x| x.is_finite() && *x > 0.0).collect();
    let mut out = FrameStats { frames: v.len(), ..Default::default() };
    let dropped = frame_times_ms.len() - v.len();
    if dropped > 0 {
        out.notes.push(format!("{dropped} invalid frame sample(s) ignored"));
    }
    out.duration_ms = v.iter().sum();
    if v.len() < MIN_FRAMES {
        out.notes.push(format!("Not enough frames yet ({} of {MIN_FRAMES})", v.len()));
        return out;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len() as f64;
    let mean = out.duration_ms / n;
    out.avg_frame_time_ms = Some(mean);
    out.avg_fps = Some(1000.0 * n / out.duration_ms);
    out.min_frame_time_ms = v.first().copied();
    out.max_frame_time_ms = v.last().copied();
    out.p50_ms = percentile(&v, 50.0);
    out.p90_ms = percentile(&v, 90.0);
    out.p95_ms = percentile(&v, 95.0);
    out.p99_ms = percentile(&v, 99.0);
    out.p999_ms = percentile(&v, 99.9);
    out.low_1pct_fps = low_fps(&v, 10);
    out.low_01pct_fps = low_fps(&v, 1);
    if out.low_1pct_fps.is_none() {
        out.notes.push(format!("1% low needs {} frames, have {}", frames_needed(10), v.len()));
    }
    if out.low_01pct_fps.is_none() {
        out.notes.push(format!("0.1% low needs {} frames, have {}", frames_needed(1), v.len()));
    }
    // Percentile FPS follows the same sample floor as the matching low.
    if out.low_1pct_fps.is_some() {
        out.p1_fps = out.p99_ms.map(|p| 1000.0 / p);
    }
    if out.low_01pct_fps.is_some() {
        out.p01_fps = out.p999_ms.map(|p| 1000.0 / p);
    }
    let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;
    out.stddev_ms = Some(var.sqrt());
    if let (Some(p50), Some(p99)) = (out.p50_ms, out.p99_ms) {
        if p99 > 0.0 {
            out.consistency_pct = Some((p50 / p99 * 100.0).min(100.0));
        }
        let limit = (p50 * 2.0).max(p50 + 4.0);
        out.stutter_frames = Some(v.iter().filter(|x| **x > limit).count());
    }
    out
}

/// Mean of the present values; `None` when there are none.
pub fn mean_opt(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let (mut sum, mut n) = (0.0, 0usize);
    for v in values.flatten() {
        if v.is_finite() {
            sum += v;
            n += 1;
        }
    }
    (n > 0).then(|| sum / n as f64)
}

/// Keep at most `max` points by taking the maximum of each bucket, so spikes
/// survive down-sampling for a frame-time graph.
pub fn downsample_max(values: &[f64], max: usize) -> Vec<f64> {
    if max == 0 || values.len() <= max {
        return values.to_vec();
    }
    let bucket = values.len().div_ceil(max);
    values.chunks(bucket).map(|c| c.iter().copied().fold(f64::MIN, f64::max)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_is_time_weighted() {
        // 50 frames at 10 ms and 50 at 30 ms: 100 frames in 2000 ms = 50 FPS,
        // not the 66.7 a mean of per-frame FPS would give.
        let mut v = vec![10.0; 50];
        v.extend(vec![30.0; 50]);
        let s = compute(&v);
        assert!((s.avg_fps.unwrap() - 50.0).abs() < 1e-9);
        assert_eq!(s.frames, 100);
    }

    #[test]
    fn percentile_interpolates() {
        let v = [10.0, 20.0, 30.0, 40.0];
        assert_eq!(percentile(&v, 0.0), Some(10.0));
        assert_eq!(percentile(&v, 100.0), Some(40.0));
        assert!((percentile(&v, 50.0).unwrap() - 25.0).abs() < 1e-9);
        assert_eq!(percentile(&[], 50.0), None);
    }

    #[test]
    fn lows_need_enough_frames() {
        let s = compute(&vec![16.0; 299]);
        assert!(s.low_1pct_fps.is_none() && s.p1_fps.is_none());
        assert!(s.notes.iter().any(|n| n.contains("1% low needs 300")));
        let s = compute(&vec![16.0; 300]);
        assert!((s.low_1pct_fps.unwrap() - 62.5).abs() < 1e-9);
        assert!(s.low_01pct_fps.is_none());
        let s = compute(&vec![16.0; 3000]);
        assert!(s.low_01pct_fps.is_some() && s.p01_fps.is_some());
    }

    #[test]
    fn one_percent_low_is_average_of_slowest() {
        // 990 frames at 10 ms, 10 frames at 50 ms → slowest 1% = the ten 50 ms frames = 20 FPS.
        let mut v = vec![10.0; 990];
        v.extend(vec![50.0; 10]);
        let s = compute(&v);
        assert!((s.low_1pct_fps.unwrap() - 20.0).abs() < 1e-9);
        assert_eq!(s.stutter_frames, Some(10));
        assert!(s.consistency_pct.unwrap() < 100.0);
    }

    #[test]
    fn too_few_frames_reports_nothing() {
        let s = compute(&[16.0, 17.0]);
        assert!(s.avg_fps.is_none());
        assert!(!s.notes.is_empty());
    }

    #[test]
    fn invalid_samples_are_dropped() {
        let mut v = vec![10.0; 20];
        v.extend([f64::NAN, -1.0, 0.0, f64::INFINITY]);
        let s = compute(&v);
        assert_eq!(s.frames, 20);
        assert!(s.notes[0].contains("4 invalid"));
    }

    #[test]
    fn downsample_keeps_spikes() {
        let mut v = vec![1.0; 1000];
        v[517] = 99.0;
        let d = downsample_max(&v, 100);
        assert!(d.len() <= 100);
        assert!(d.contains(&99.0));
    }
}
