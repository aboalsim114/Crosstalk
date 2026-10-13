//! "Focus voix" (cahier des charges A-01, experimental): CS2's voice chat is
//! played in the centre (same signal left and right) while shots, steps and
//! explosions are spatialised (louder on one side, slightly delayed on the
//! other). For each frequency of each 21 ms frame, the left and right
//! spectra are compared: what is identical on both sides (the voice) is
//! kept, what differs (game sounds coming from a side) is turned down.
//! Sounds exactly in the centre (one's own shots) cannot be told apart and
//! stay.
//!
//! Runs on the capture thread before the 16 kHz conversion (two 1024-point
//! FFTs per 10.7 ms); adds 10.7 ms of latency.

use std::f32::consts::PI;

/// Frame length at 48 kHz (21 ms) and hop (50 % overlap).
const N: usize = 1024;
const HOP: usize = N / 2;
/// Strongest attenuation of a bin (−20 dB).
const FLOOR: f32 = 0.1;
/// Sharpness of the centre selection.
const POWER: i32 = 3;

/// Output delay, in frames.
pub const LATENCY_FRAMES: usize = N - HOP;

#[derive(Clone, Copy, Default, Debug)]
struct C {
    re: f32,
    im: f32,
}

/// In-place radix-2 FFT (`inverse`: unscaled inverse).
fn fft(x: &mut [C], twiddles: &[C], inverse: bool) {
    let n = x.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            x.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let step = n / len;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let mut w = twiddles[k * step];
                if inverse {
                    w.im = -w.im;
                }
                let a = x[start + k];
                let b = x[start + k + len / 2];
                let t = C {
                    re: b.re * w.re - b.im * w.im,
                    im: b.re * w.im + b.im * w.re,
                };
                x[start + k] = C {
                    re: a.re + t.re,
                    im: a.im + t.im,
                };
                x[start + k + len / 2] = C {
                    re: a.re - t.re,
                    im: a.im - t.im,
                };
            }
        }
        len <<= 1;
    }
}

#[derive(Debug, Clone)]
pub struct VoiceFocus {
    window: Vec<f32>,
    twiddles: Vec<C>,
    /// Last `N` input frames, per channel.
    left: Vec<f32>,
    right: Vec<f32>,
    /// New frames since the last FFT.
    fill: usize,
    /// Overlap-add accumulator.
    acc: Vec<f32>,
    gains: Vec<f32>,
    spec_l: Vec<C>,
    spec_r: Vec<C>,
    /// Energy in / out since the last reading (for the "removed" meter).
    pub energy_in: f64,
    pub energy_out: f64,
}

impl Default for VoiceFocus {
    fn default() -> Self {
        // sqrt-Hann analysis and synthesis: sums to 1 at 50 % overlap.
        let window = (0..N)
            .map(|i| (0.5 - 0.5 * (2.0 * PI * i as f32 / N as f32).cos()).sqrt())
            .collect();
        let twiddles = (0..N / 2)
            .map(|k| {
                let a = -2.0 * PI * k as f32 / N as f32;
                C {
                    re: a.cos(),
                    im: a.sin(),
                }
            })
            .collect();
        Self {
            window,
            twiddles,
            left: vec![0.0; N],
            right: vec![0.0; N],
            fill: 0,
            acc: vec![0.0; N],
            gains: vec![1.0; N / 2 + 1],
            spec_l: vec![C::default(); N],
            spec_r: vec![C::default(); N],
            energy_in: 0.0,
            energy_out: 0.0,
        }
    }
}

/// Gain of a bin from how alike left and right are: 1 when identical (the
/// centre), down to `FLOOR` when one side dominates or they are out of phase.
fn centre_gain(l: C, r: C) -> f32 {
    let p = l.re * l.re + l.im * l.im + r.re * r.re + r.im * r.im;
    if p < 1e-12 {
        return 1.0;
    }
    // 2·Re(L·R*) / (|L|² + |R|²): 1 centre, 0 one side only, < 0 out of phase.
    let c = (2.0 * (l.re * r.re + l.im * r.im) / p).clamp(0.0, 1.0);
    c.powi(POWER).max(FLOOR)
}

impl VoiceFocus {
    fn run_frame(&mut self) {
        for i in 0..N {
            self.spec_l[i] = C {
                re: self.left[i] * self.window[i],
                im: 0.0,
            };
            self.spec_r[i] = C {
                re: self.right[i] * self.window[i],
                im: 0.0,
            };
        }
        fft(&mut self.spec_l, &self.twiddles, false);
        fft(&mut self.spec_r, &self.twiddles, false);
        let mut mid = std::mem::take(&mut self.spec_l);
        for k in 0..=N / 2 {
            let (l, r) = (mid[k], self.spec_r[k]);
            let target = centre_gain(l, r);
            // Fast attack, slower release: fewer "musical" artefacts.
            let g = &mut self.gains[k];
            *g = if target < *g {
                target
            } else {
                *g + 0.5 * (target - *g)
            };
            let m = C {
                re: 0.5 * (l.re + r.re) * *g,
                im: 0.5 * (l.im + r.im) * *g,
            };
            mid[k] = m;
            if k > 0 && k < N / 2 {
                mid[N - k] = C {
                    re: m.re,
                    im: -m.im,
                };
            }
        }
        fft(&mut mid, &self.twiddles, true);
        let scale = 1.0 / N as f32;
        for ((a, m), w) in self.acc.iter_mut().zip(&mid).zip(&self.window) {
            *a += m.re * scale * w;
        }
        self.spec_l = mid;
    }

    /// Interleaved stereo to cleaned mono (delayed by `LATENCY_FRAMES`);
    /// other layouts are averaged.
    pub fn process(&mut self, samples: &[f32], channels: u16, out: &mut Vec<f32>) {
        if channels != 2 {
            out.extend(crate::audio::convert::mono_frames(samples, channels));
            return;
        }
        out.reserve(samples.len() / 2);
        for f in samples.chunks_exact(2) {
            let at = N - HOP + self.fill;
            self.left[at] = f[0];
            self.right[at] = f[1];
            let m = 0.5 * (f[0] + f[1]);
            self.energy_in += (m * m) as f64;
            self.fill += 1;
            if self.fill == HOP {
                self.run_frame();
                for &y in &self.acc[..HOP] {
                    self.energy_out += (y * y) as f64;
                    out.push(y);
                }
                self.acc.copy_within(HOP.., 0);
                self.acc[N - HOP..].fill(0.0);
                self.left.copy_within(HOP.., 0);
                self.right.copy_within(HOP.., 0);
                self.fill = 0;
            }
        }
    }

    /// dB removed since the last call, and reset; `None` without signal.
    pub fn take_reduction_db(&mut self) -> Option<f32> {
        let (i, o) = (self.energy_in, self.energy_out);
        self.energy_in = 0.0;
        self.energy_out = 0.0;
        (i > 1e-6).then(|| (10.0 * (i / o.max(1e-12)).log10()).max(0.0) as f32)
    }
}

/// Signal-to-noise ratio of `out` against the clean voice it should be
/// (aligned by `delay` samples), in dB.
pub fn snr_db(clean: &[f32], out: &[f32], delay: usize) -> f32 {
    let n = clean.len().min(out.len().saturating_sub(delay));
    let (mut sig, mut err) = (0.0f64, 0.0f64);
    for i in 0..n {
        let c = clean[i] as f64;
        let d = out[i + delay] as f64 - c;
        sig += c * c;
        err += d * d;
    }
    (10.0 * (sig / err.max(1e-12)).log10()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, seed: u64) -> Vec<f32> {
        let mut x = seed.max(1);
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x % 20_000) as f32 / 10_000.0 - 1.0
            })
            .collect()
    }

    fn tone(n: usize, f: f32) -> Vec<f32> {
        (0..n)
            .map(|i| 0.3 * (2.0 * PI * f * i as f32 / 48_000.0).sin())
            .collect()
    }

    /// Voice in the centre plus a noise panned to one side with a delay.
    fn mix(voice: &[f32], noise: &[f32], gl: f32, gr: f32, itd: usize) -> Vec<f32> {
        let mut st = Vec::with_capacity(voice.len() * 2);
        for i in 0..voice.len() {
            let nl = gl * noise[i];
            let nr = if i >= itd { gr * noise[i - itd] } else { 0.0 };
            st.push(voice[i] + nl);
            st.push(voice[i] + nr);
        }
        st
    }

    fn run(st: &[f32]) -> Vec<f32> {
        let mut f = VoiceFocus::default();
        let mut out = Vec::new();
        for block in st.chunks(441 * 2) {
            f.process(block, 2, &mut out);
        }
        out
    }

    #[test]
    fn fft_round_trip() {
        let v = VoiceFocus::default();
        let mut x: Vec<C> = (0..N)
            .map(|i| C {
                re: (i as f32 * 0.37).sin(),
                im: 0.0,
            })
            .collect();
        let orig = x.clone();
        fft(&mut x, &v.twiddles, false);
        fft(&mut x, &v.twiddles, true);
        for (a, b) in x.iter().zip(&orig) {
            assert!((a.re / N as f32 - b.re).abs() < 1e-4);
        }
    }

    #[test]
    fn a_centred_voice_passes_unchanged() {
        let v = tone(48_000, 440.0);
        let st: Vec<f32> = v.iter().flat_map(|&s| [s, s]).collect();
        let out = run(&st);
        assert!(snr_db(&v[N..40_000], &out[N + LATENCY_FRAMES..], 0) > 50.0);
    }

    #[test]
    fn a_sound_on_one_side_is_removed_from_the_voice() {
        let n = 48_000 * 2;
        let v = tone(n, 300.0);
        let nz = noise(n, 7);
        // Louder on the left, 0.5 ms later on the right (24 samples).
        let st = mix(&v, &nz, 0.5, 0.15, 24);
        let plain: Vec<f32> = st.chunks(2).map(|f| 0.5 * (f[0] + f[1])).collect();
        let out = run(&st);
        let before = snr_db(&v[N..], &plain[N..], 0);
        let after = snr_db(&v[N..n - N], &out[N + LATENCY_FRAMES..], 0);
        assert!(
            after > before + 8.0,
            "before {before:.1} dB, after {after:.1} dB"
        );
    }

    #[test]
    fn mono_and_other_layouts_are_averaged() {
        let mut f = VoiceFocus::default();
        let mut out = Vec::new();
        f.process(&[0.2, 0.4, 0.6], 1, &mut out);
        f.process(&[0.0, 0.3, 0.6, 0.0, 0.0, 0.0], 3, &mut out);
        assert_eq!(out.len(), 5);
        assert!((out[3] - 0.3).abs() < 1e-6);
        assert_eq!(f.take_reduction_db(), None);
    }
}
