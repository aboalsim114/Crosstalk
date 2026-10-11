//! Signal level metering for the UI indicator.

use super::convert::mono_frames;
use super::types::{AudioBlock, LevelReading};

/// Floor of the dB scale: anything quieter is reported as this value.
pub const DB_FLOOR: f32 = -90.0;
/// Readings per second sent to the UI.
pub const READINGS_PER_SECOND: u32 = 15;

pub fn to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        DB_FLOOR
    } else {
        (20.0 * linear.log10()).max(DB_FLOOR)
    }
}

/// Accumulates RMS and peak over windows of `sample_rate / 15` frames. Windows
/// are counted in frames, not wall-clock time, so the output is deterministic.
#[derive(Debug, Default)]
pub struct LevelMeter {
    sum_sq: f64,
    peak: f32,
    frames: u32,
}

impl LevelMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds a block; calls `emit` for every completed window.
    pub fn push(&mut self, block: &AudioBlock, mut emit: impl FnMut(LevelReading)) {
        let window = (block.sample_rate / READINGS_PER_SECOND).max(1);
        for s in mono_frames(block.samples, block.channels) {
            self.sum_sq += (s as f64) * (s as f64);
            self.peak = self.peak.max(s.abs());
            self.frames += 1;
            if self.frames >= window {
                emit(self.take());
            }
        }
    }

    /// Feeds `frames` frames of silence (WASAPI "silent" buffers).
    pub fn push_silence(
        &mut self,
        frames: usize,
        sample_rate: u32,
        mut emit: impl FnMut(LevelReading),
    ) {
        let window = (sample_rate / READINGS_PER_SECOND).max(1);
        let mut left = frames as u32;
        while left > 0 {
            // `frames < window` always holds between pushes.
            let n = left.min(window - self.frames);
            self.frames += n;
            left -= n;
            if self.frames >= window {
                emit(self.take());
            }
        }
    }

    fn take(&mut self) -> LevelReading {
        let rms = if self.frames == 0 {
            0.0
        } else {
            (self.sum_sq / self.frames as f64).sqrt() as f32
        };
        let peak = self.peak.min(1.0);
        *self = Self::default();
        LevelReading {
            rms: rms.min(1.0),
            peak,
            rms_db: to_db(rms),
            peak_db: to_db(peak),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(samples: &[f32], channels: u16, rate: u32) -> AudioBlock<'_> {
        AudioBlock {
            samples,
            channels,
            sample_rate: rate,
            qpc_100ns: 0,
        }
    }

    #[test]
    fn db_scale() {
        assert_eq!(to_db(0.0), DB_FLOOR);
        assert!((to_db(1.0)).abs() < 1e-6);
        assert!((to_db(0.5) + 6.0206).abs() < 1e-3);
        assert_eq!(to_db(1e-9), DB_FLOOR);
    }

    #[test]
    fn full_scale_sine_reads_minus_3_db() {
        let rate = 48_000;
        let sine: Vec<f32> = (0..rate)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 1000.0 / rate as f32).sin())
            .collect();
        let mut readings = Vec::new();
        LevelMeter::new().push(&block(&sine, 1, rate), |r| readings.push(r));
        assert_eq!(readings.len(), READINGS_PER_SECOND as usize);
        for r in readings {
            assert!((r.rms_db + 3.01).abs() < 0.1, "rms {}", r.rms_db);
            assert!(r.peak > 0.99);
        }
    }

    #[test]
    fn stereo_is_downmixed_before_metering() {
        // Opposite-phase channels cancel out in the mono mix.
        let rate = 1_500;
        let samples: Vec<f32> = (0..rate).flat_map(|_| [0.5, -0.5]).collect();
        let mut readings = Vec::new();
        LevelMeter::new().push(&block(&samples, 2, rate), |r| readings.push(r));
        assert!(readings.iter().all(|r| r.rms_db == DB_FLOOR));
    }

    #[test]
    fn windows_span_block_boundaries() {
        let rate = 1_500; // window = 100 frames
        let chunk = [0.25f32; 60];
        let mut meter = LevelMeter::new();
        let mut readings = Vec::new();
        for _ in 0..5 {
            meter.push(&block(&chunk, 1, rate), |r| readings.push(r));
        }
        assert_eq!(readings.len(), 3); // 300 frames
        assert!((readings[0].rms - 0.25).abs() < 1e-6);
    }

    #[test]
    fn silence_produces_floor_readings() {
        let mut meter = LevelMeter::new();
        let mut readings = Vec::new();
        meter.push_silence(48_000, 48_000, |r| readings.push(r));
        assert_eq!(readings.len(), 15);
        assert!(readings
            .iter()
            .all(|r| r.peak == 0.0 && r.rms_db == DB_FLOOR));
    }

    #[test]
    fn clips_are_capped_at_full_scale() {
        let loud = [2.0f32; 100];
        let mut readings = Vec::new();
        LevelMeter::new().push(&block(&loud, 1, 1_500), |r| readings.push(r));
        assert_eq!(readings[0].peak, 1.0);
        assert_eq!(readings[0].rms, 1.0);
    }
}
