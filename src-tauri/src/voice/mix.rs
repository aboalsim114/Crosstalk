//! Audio sent to the virtual cable: the real microphone passed through, plus
//! the AI voice when it speaks. Pure functions and a small queue, tested on
//! every platform; the Windows router (router.rs) only moves the buffers.

// Used by the Windows router; elsewhere only by the tests.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::VecDeque;
use std::sync::Mutex;

/// Linear resampling (speech, short sentences: enough, and no latency).
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() || from == 0 || to == 0 {
        return input.to_vec();
    }
    let n = (input.len() as u64 * to as u64 / from as u64) as usize;
    let step = from as f64 / to as f64;
    (0..n)
        .map(|i| {
            let pos = i as f64 * step;
            let j = pos as usize;
            let frac = (pos - j as f64) as f32;
            let a = input[j.min(input.len() - 1)];
            let b = input[(j + 1).min(input.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

/// Mono samples waiting to be played, at the output rate. Bounded so that a
/// stuck output never grows memory.
pub struct Queue {
    buf: Mutex<VecDeque<f32>>,
    max: usize,
}

impl Queue {
    pub fn new(max: usize) -> Self {
        Self {
            buf: Mutex::new(VecDeque::new()),
            max,
        }
    }

    /// Adds samples; the oldest are dropped beyond the bound.
    pub fn push(&self, samples: &[f32]) {
        let mut b = self.buf.lock().unwrap();
        b.extend(samples.iter().copied());
        let over = b.len().saturating_sub(self.max);
        b.drain(..over);
    }

    /// Up to `n` samples (fewer when the queue runs dry).
    pub fn take(&self, n: usize) -> Vec<f32> {
        let mut b = self.buf.lock().unwrap();
        let k = n.min(b.len());
        b.drain(..k).collect()
    }

    pub fn len(&self) -> usize {
        self.buf.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// `n` mono output samples: microphone + voice, each with its gain, clipped.
pub fn mix(mic: &[f32], voice: &[f32], n: usize, mic_gain: f32, voice_gain: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let m = mic.get(i).copied().unwrap_or(0.0) * mic_gain;
            let v = voice.get(i).copied().unwrap_or(0.0) * voice_gain;
            (m + v).clamp(-1.0, 1.0)
        })
        .collect()
}

/// Volume in percent (0–150) → gain, with a gentle curve.
pub fn gain(percent: u32) -> f32 {
    (percent.min(150) as f32 / 100.0).powf(1.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_piper_rate_to_the_device_rate() {
        let one_sec: Vec<f32> = (0..22_050).map(|i| i as f32 / 22_050.0).collect();
        let out = resample(&one_sec, 22_050, 48_000);
        assert_eq!(out.len(), 48_000);
        assert!((out[24_000] - 0.5).abs() < 0.001);
        assert_eq!(resample(&[0.1, 0.2], 48_000, 48_000), vec![0.1, 0.2]);
        assert!(resample(&[], 16_000, 48_000).is_empty());
    }

    #[test]
    fn mixes_the_microphone_and_the_voice_without_clipping_overflow() {
        let out = mix(&[0.5, 0.9], &[0.25, 0.9, 0.3], 4, 1.0, 1.0);
        assert_eq!(out, vec![0.75, 1.0, 0.3, 0.0]);
        assert_eq!(gain(100), 1.0);
        assert_eq!(gain(0), 0.0);
        assert!(gain(500) <= gain(150));
    }

    #[test]
    fn the_queue_is_bounded_and_drains_in_order() {
        let q = Queue::new(4);
        q.push(&[1.0, 2.0, 3.0]);
        q.push(&[4.0, 5.0, 6.0]);
        assert_eq!(q.len(), 4);
        assert_eq!(q.take(3), vec![3.0, 4.0, 5.0]);
        assert_eq!(q.take(5), vec![6.0]);
        assert!(q.is_empty());
    }
}
