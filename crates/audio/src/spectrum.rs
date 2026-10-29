//! Turns the last few thousand output samples into a handful of bands for
//! the visualiser. Cheap: one 2048-point real FFT per call.

use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

pub struct Analyser {
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    input: Vec<f32>,
    output: Vec<realfft::num_complex::Complex<f32>>,
    scratch: Vec<realfft::num_complex::Complex<f32>>,
}

impl Analyser {
    pub fn new(n: usize) -> Analyser {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(n);
        // Hann window, so the edges of the slice don't smear into every band.
        let window = (0..n).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (n - 1) as f32).cos()).collect();
        let input = fft.make_input_vec();
        let output = fft.make_output_vec();
        let scratch = fft.make_scratch_vec();
        Analyser { fft, window, input, output, scratch }
    }

    /// `bands` levels from 0 (silent) to 1 (loud), low notes first, spaced
    /// like hearing is (logarithmically) from 40 Hz up to 16 kHz.
    pub fn bands(&mut self, samples: &[f32], rate: u32, bands: usize) -> Vec<f32> {
        let n = self.window.len();
        if bands == 0 {
            return Vec::new();
        }
        if samples.len() < n / 2 || rate == 0 {
            return vec![0.0; bands];
        }
        // Right-align the newest samples, zero-padding the front if short.
        let pad = n.saturating_sub(samples.len());
        let src = &samples[samples.len().saturating_sub(n)..];
        for (i, v) in self.input.iter_mut().enumerate() {
            *v = if i < pad { 0.0 } else { src[i - pad] * self.window[i] };
        }
        if self.fft.process_with_scratch(&mut self.input, &mut self.output, &mut self.scratch).is_err() {
            return vec![0.0; bands];
        }
        let bin_hz = rate as f32 / n as f32;
        let lo = 40f32;
        let hi = 16_000f32.min(rate as f32 / 2.0);
        let ratio = (hi / lo).ln();
        // A full-scale sine through a Hann window peaks at about n/4.
        let scale = 4.0 / n as f32;
        (0..bands)
            .map(|b| {
                let f0 = lo * (ratio * b as f32 / bands as f32).exp();
                let f1 = lo * (ratio * (b + 1) as f32 / bands as f32).exp();
                let i0 = ((f0 / bin_hz) as usize).max(1);
                let i1 = ((f1 / bin_hz).ceil() as usize).max(i0 + 1).min(self.output.len());
                let peak = self.output[i0.min(self.output.len() - 1)..i1].iter().map(|c| c.norm()).fold(0.0f32, f32::max) * scale;
                // −66 dB … −6 dB onto 0 … 1.
                let db = 20.0 * peak.max(1e-9).log10();
                ((db + 66.0) / 60.0).clamp(0.0, 1.0)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_lights_up_its_own_band() {
        let rate = 48_000;
        let tone: Vec<f32> = (0..2048).map(|i| (std::f32::consts::TAU * 1000.0 * i as f32 / rate as f32).sin() * 0.5).collect();
        let mut a = Analyser::new(2048);
        let b = a.bands(&tone, rate, 32);
        let loudest = b.iter().enumerate().max_by(|x, y| x.1.total_cmp(y.1)).unwrap().0;
        // 1 kHz on a 40 Hz–16 kHz log scale sits a little past halfway.
        let expect = ((1000f32 / 40.0).ln() / (16_000f32 / 40.0).ln() * 32.0) as usize;
        assert!(loudest.abs_diff(expect) <= 1, "loudest band {loudest}, expected about {expect}");
        assert!(b[loudest] > 0.7);
        assert!(b[0] < 0.3 && b[31] < 0.3);
        assert!(a.bands(&vec![0.0; 2048], rate, 8).iter().all(|&v| v == 0.0));
    }
}
