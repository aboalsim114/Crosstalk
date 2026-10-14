//! Light signal conditioning before the VAD and Whisper: resampling of the
//! captured stream to 16 kHz mono and a high-pass filter against the rumble
//! of explosions. No denoising: Whisper copes better with the raw voice.

use std::f64::consts::PI;

/// Sample rate expected by Silero VAD and Whisper.
pub const SAMPLE_RATE: u32 = 16_000;

/// Streaming windowed-sinc resampler (any input rate to `SAMPLE_RATE`).
///
/// 32 taps with a Blackman window: about 70 dB of alias rejection, plenty for
/// speech recognition, at a cost of ~0.5 M multiply-adds per second.
pub struct Resampler {
    /// Input samples per output sample.
    step: f64,
    /// Anti-alias cut-off relative to the input Nyquist frequency.
    cutoff: f64,
    passthrough: bool,
    buf: Vec<f32>,
    /// Position of the next output sample in `buf`.
    t: f64,
}

const HALF_TAPS: usize = 16;

impl Resampler {
    pub fn new(input_rate: u32) -> Self {
        let step = input_rate.max(1) as f64 / SAMPLE_RATE as f64;
        Self {
            step,
            // Keep a little margin below the output Nyquist frequency.
            cutoff: (1.0 / step).min(1.0) * 0.94,
            passthrough: input_rate == SAMPLE_RATE,
            buf: vec![0.0; HALF_TAPS],
            t: HALF_TAPS as f64,
        }
    }

    /// Resamples `input` and appends the result to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.passthrough {
            out.extend_from_slice(input);
            return;
        }
        self.buf.extend_from_slice(input);
        let len = self.buf.len();
        while (self.t.floor() as usize) + HALF_TAPS < len {
            let base = self.t.floor() as usize;
            let frac = self.t - base as f64;
            let mut acc = 0.0f64;
            for j in 0..2 * HALF_TAPS {
                let k = base + j + 1 - HALF_TAPS;
                let d = frac + HALF_TAPS as f64 - 1.0 - j as f64;
                acc += self.buf[k] as f64 * self.kernel(d);
            }
            out.push(acc as f32);
            self.t += self.step;
        }
        // Keep only the history the next outputs need.
        let keep_from = (self.t.floor() as usize).saturating_sub(HALF_TAPS - 1);
        if keep_from > 0 {
            self.buf.drain(..keep_from);
            self.t -= keep_from as f64;
        }
    }

    fn kernel(&self, d: f64) -> f64 {
        let x = d / HALF_TAPS as f64;
        if x.abs() >= 1.0 {
            return 0.0;
        }
        let window = 0.42 + 0.5 * (PI * x).cos() + 0.08 * (2.0 * PI * x).cos();
        let arg = PI * self.cutoff * d;
        let sinc = if arg.abs() < 1e-9 {
            1.0
        } else {
            arg.sin() / arg
        };
        self.cutoff * sinc * window
    }
}

/// Second-order Butterworth high-pass (RBJ biquad).
pub struct HighPass {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

/// Below the fundamental of most voices; removes explosion rumble and hum.
pub const HIGH_PASS_HZ: f32 = 120.0;

impl HighPass {
    pub fn new(cutoff_hz: f32, sample_rate: u32) -> Self {
        Self::with_q(cutoff_hz, sample_rate, std::f32::consts::FRAC_1_SQRT_2)
    }

    /// Any Q (cascades of these make steeper Butterworth filters).
    pub fn with_q(cutoff_hz: f32, sample_rate: u32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * cutoff_hz / sample_rate as f32;
        let alpha = w0.sin() / (2.0 * q);
        let cos = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b: [
                (1.0 + cos) / 2.0 / a0,
                -(1.0 + cos) / a0,
                (1.0 + cos) / 2.0 / a0,
            ],
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for s in samples {
            let x0 = *s;
            let y0 = self.b[0] * x0 + self.b[1] * self.x[0] + self.b[2] * self.x[1]
                - self.a[0] * self.y[0]
                - self.a[1] * self.y[1];
            self.x = [x0, self.x[0]];
            self.y = [y0, self.y[0]];
            *s = y0;
        }
    }
}

/// RMS level of `samples` in dBFS (floored at -90).
pub fn rms_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -90.0;
    }
    let mean = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    (10.0 * mean.max(1e-9).log10()).max(-90.0)
}

/// 16 kHz mono conditioning chain: resampler then high-pass.
pub struct Conditioner {
    input_rate: u32,
    resampler: Resampler,
    high_pass: HighPass,
}

impl Conditioner {
    pub fn new(input_rate: u32) -> Self {
        Self {
            input_rate,
            resampler: Resampler::new(input_rate),
            high_pass: HighPass::new(HIGH_PASS_HZ, SAMPLE_RATE),
        }
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    /// Converts mono samples at the input rate; appends 16 kHz samples to `out`.
    pub fn process(&mut self, mono: &[f32], out: &mut Vec<f32>) {
        let start = out.len();
        self.resampler.process(mono, out);
        self.high_pass.process(&mut out[start..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: u32, secs: f32) -> Vec<f32> {
        (0..(rate as f32 * secs) as usize)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin() * 0.5)
            .collect()
    }

    fn resample_all(input: &[f32], rate: u32, chunk: usize) -> Vec<f32> {
        let mut r = Resampler::new(rate);
        let mut out = Vec::new();
        for c in input.chunks(chunk) {
            r.process(c, &mut out);
        }
        out
    }

    /// Amplitude of `freq` in `x` (16 kHz) by correlation.
    fn tone_amplitude(x: &[f32], freq: f32) -> f32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in x.iter().enumerate() {
            let ph = 2.0 * std::f64::consts::PI * freq as f64 * i as f64 / SAMPLE_RATE as f64;
            re += *s as f64 * ph.cos();
            im += *s as f64 * ph.sin();
        }
        (2.0 * (re * re + im * im).sqrt() / x.len() as f64) as f32
    }

    #[test]
    fn resamples_common_rates_to_16k_with_the_right_length() {
        for rate in [48_000, 44_100, 32_000, 22_050, 96_000] {
            let out = resample_all(&sine(440.0, rate, 1.0), rate, 480);
            let n = out.len() as i64;
            assert!((n - 16_000).abs() <= HALF_TAPS as i64 + 1, "{rate}: {n}");
        }
    }

    #[test]
    fn keeps_voice_band_tones_and_rejects_aliases() {
        let out = resample_all(&sine(1_000.0, 48_000, 1.0), 48_000, 441);
        let body = &out[1000..15000];
        assert!((tone_amplitude(body, 1_000.0) - 0.5).abs() < 0.02);

        // 12 kHz cannot be represented at 16 kHz: it must not fold to 4 kHz.
        let out = resample_all(&sine(12_000.0, 48_000, 1.0), 48_000, 512);
        let body = &out[1000..15000];
        assert!(tone_amplitude(body, 4_000.0) < 0.005);
        assert!(rms_db(body) < -45.0, "{}", rms_db(body));
    }

    #[test]
    fn chunking_does_not_change_the_output() {
        let x = sine(700.0, 44_100, 0.5);
        let a = resample_all(&x, 44_100, 100);
        let b = resample_all(&x, 44_100, 4096);
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(p, q)| (p - q).abs() < 1e-5));
    }

    #[test]
    fn passes_16k_through() {
        let x = sine(300.0, SAMPLE_RATE, 0.1);
        assert_eq!(resample_all(&x, SAMPLE_RATE, 64), x);
    }

    #[test]
    fn high_pass_removes_rumble_and_keeps_voice() {
        let mut low = sine(40.0, SAMPLE_RATE, 1.0);
        let mut voice = sine(500.0, SAMPLE_RATE, 1.0);
        let mut hp = HighPass::new(HIGH_PASS_HZ, SAMPLE_RATE);
        hp.process(&mut low);
        let mut hp = HighPass::new(HIGH_PASS_HZ, SAMPLE_RATE);
        hp.process(&mut voice);
        assert!(rms_db(&low[4000..]) < rms_db(&sine(40.0, SAMPLE_RATE, 1.0)) - 15.0);
        assert!((rms_db(&voice[4000..]) - rms_db(&sine(500.0, SAMPLE_RATE, 1.0))).abs() < 0.5);
    }

    #[test]
    fn rms_of_silence_is_the_floor() {
        assert_eq!(rms_db(&[]), -90.0);
        assert_eq!(rms_db(&[0.0; 10]), -90.0);
        assert!((rms_db(&[1.0, -1.0]) - 0.0).abs() < 1e-4);
    }
}
