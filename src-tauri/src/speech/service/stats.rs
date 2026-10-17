//! Speech statistics (Audio & IA, diagnostic): segments, rejections,
//! latency percentiles, real-time factor and CPU.

use std::collections::{BTreeMap, VecDeque};

use crate::cpu::CpuMeter;
use crate::speech::types::{Percentiles, RejectReason, SpeechStats, StageLatency};

const LATENCY_WINDOW: usize = 100;

#[derive(Default)]
pub(super) struct StatsAcc {
    pub(super) segments: u64,
    pub(super) transcribed: u64,
    pub(super) rejected: BTreeMap<RejectReason, u64>,
    pub(super) latencies: VecDeque<u32>,
    /// Same lines, per stage: closing, queue, recognition.
    pub(super) stages: [VecDeque<u32>; 3],
    pub(super) last_rtf: Option<f32>,
    /// Recogniser time and audio duration of every final segment transcribed.
    pub(super) asr_ms: u64,
    pub(super) asr_audio_ms: u64,
    pub(super) cpu: CpuMeter,
    pub(super) cpu_percent: Option<f32>,
}

impl StatsAcc {
    pub(super) fn reject(&mut self, r: RejectReason) {
        *self.rejected.entry(r).or_default() += 1;
    }

    /// A transcript shown (final): counted, and its latency kept for the
    /// percentiles (last `LATENCY_WINDOW` lines).
    pub(super) fn transcribed(&mut self, latency_ms: u32, stages: [u32; 3]) {
        self.transcribed += 1;
        push(&mut self.latencies, latency_ms);
        for (w, v) in self.stages.iter_mut().zip(stages) {
            push(w, v);
        }
    }

    pub(super) fn snapshot(&self, dropped_blocks: u64) -> SpeechStats {
        let total = percentiles(&self.latencies);
        let [close, queue, asr] = self.stages.each_ref().map(percentiles);
        SpeechStats {
            segments: self.segments,
            transcribed: self.transcribed,
            rejected: self.rejected.clone(),
            dropped_blocks,
            latency_p50_ms: total.p50_ms,
            latency_p95_ms: total.p95_ms,
            last_rtf: self.last_rtf,
            asr_ms: self.asr_ms,
            asr_audio_ms: self.asr_audio_ms,
            cpu_percent: self.cpu_percent,
            focus_db: None,
            stages: StageLatency { close, queue, asr },
        }
    }
}

fn push(window: &mut VecDeque<u32>, v: u32) {
    window.push_back(v);
    if window.len() > LATENCY_WINDOW {
        window.pop_front();
    }
}

fn percentiles(window: &VecDeque<u32>) -> Percentiles {
    let mut sorted: Vec<u32> = window.iter().copied().collect();
    sorted.sort_unstable();
    let pct = |q: f32| {
        (!sorted.is_empty())
            .then(|| sorted[((q * sorted.len() as f32) as usize).min(sorted.len() - 1)])
    };
    Percentiles {
        p50_ms: pct(0.5),
        p95_ms: pct(0.95),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_the_latency_by_stage() {
        let mut s = StatsAcc::default();
        for i in 0..10u32 {
            s.transcribed(1000 + i, [600, 10 * i, 300 + i]);
        }
        let snap = s.snapshot(0);
        assert_eq!(snap.latency_p50_ms, Some(1005));
        assert_eq!(
            snap.stages.close,
            Percentiles {
                p50_ms: Some(600),
                p95_ms: Some(600)
            }
        );
        assert_eq!(snap.stages.queue.p95_ms, Some(90));
        assert_eq!(snap.stages.asr.p50_ms, Some(305));
        assert_eq!(
            StatsAcc::default().snapshot(0).stages,
            StageLatency::default()
        );
    }
}
