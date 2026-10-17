//! Turns per-frame speech probabilities into utterance segments for the recogniser.
//!
//! - opens after `start_frames` frames above the threshold, with a pre-roll so
//!   the first syllable is kept;
//! - closes after a post-roll of silence (hysteresis: a lower release
//!   threshold keeps short pauses inside the segment);
//! - cuts long monologues at the quietest recent frame (8 s max);
//! - rejects segments that look like a shot or an explosion: Silero does not
//!   separate voices from game sounds perfectly.

use std::collections::VecDeque;

use super::dsp::rms_db;
use super::types::RejectReason;
use super::vad::{FRAME, FRAME_MS};

#[derive(Debug, Clone, PartialEq)]
pub struct SegmenterConfig {
    pub threshold: f32,
    /// Audio kept before the onset (8 frames = 256 ms).
    pub pre_roll_frames: usize,
    /// Silence that closes a segment, kept as post-roll (13 frames = 416 ms).
    pub hang_frames: usize,
    /// Consecutive frames above the threshold needed to open a segment.
    pub start_frames: usize,
    /// Voiced audio needed for a segment to be transcribed (256 ms).
    pub min_voiced_frames: usize,
    /// Hard cap (156 frames = 5 s): with game noise keeping a segment open,
    /// a longer cap only delays the final line.
    pub max_frames: usize,
}

impl SegmenterConfig {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold,
            pre_roll_frames: 8,
            hang_frames: 13,
            start_frames: 2,
            min_voiced_frames: 8,
            max_frames: 156,
        }
    }

    /// Frames stay "voiced" down to this probability once a segment is open.
    pub fn release(&self) -> f32 {
        (self.threshold - 0.15).max(0.1)
    }
}

/// A validated utterance, 16 kHz mono.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub samples: Vec<f32>,
    /// Index of the first frame since the stream started.
    pub start_frame: u64,
    /// Index of the last frame with a voice.
    pub last_voiced_frame: u64,
    pub voiced_frames: usize,
    /// Cut at the maximum length while the speaker kept talking.
    pub forced: bool,
}

impl Segment {
    pub fn duration_ms(&self) -> u32 {
        (self.samples.len() as u64 * 1000 / super::dsp::SAMPLE_RATE as u64) as u32
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Segment(Segment),
    Rejected(RejectReason),
}

struct Frame {
    samples: Vec<f32>,
    prob: f32,
    db: f32,
    index: u64,
}

pub struct Segmenter {
    cfg: SegmenterConfig,
    next_index: u64,
    /// Idle: the last `pre_roll_frames` frames. Active: the open segment.
    frames: VecDeque<Frame>,
    active: bool,
    above_run: usize,
    silence_run: usize,
}

impl Segmenter {
    pub fn new(cfg: SegmenterConfig) -> Self {
        Self {
            cfg,
            next_index: 0,
            frames: VecDeque::new(),
            active: false,
            above_run: 0,
            silence_run: 0,
        }
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.cfg.threshold = threshold;
    }

    pub fn config(&self) -> &SegmenterConfig {
        &self.cfg
    }

    pub fn is_speaking(&self) -> bool {
        self.active
    }

    /// Unvoiced frames at the end of the open segment: the voice may have
    /// just stopped (the segment closes after `hang_frames` of them).
    pub fn trailing_silence(&self) -> usize {
        if self.active {
            self.silence_run
        } else {
            0
        }
    }

    /// Frames of the segment being recorded (0 when nobody speaks).
    pub fn open_frames(&self) -> usize {
        if self.active {
            self.frames.len()
        } else {
            0
        }
    }

    /// The segment being recorded so far, for a live partial transcript.
    pub fn open_segment(&self) -> Option<Segment> {
        if !self.active || self.frames.is_empty() {
            return None;
        }
        let release = self.cfg.release();
        let voiced: Vec<&Frame> = self.frames.iter().filter(|f| f.prob >= release).collect();
        let mut samples = Vec::with_capacity(self.frames.len() * FRAME);
        for f in &self.frames {
            samples.extend_from_slice(&f.samples);
        }
        Some(Segment {
            samples,
            start_frame: self.frames[0].index,
            last_voiced_frame: voiced.last().map_or(self.frames[0].index, |f| f.index),
            voiced_frames: voiced.len(),
            forced: false,
        })
    }

    /// Index of the next frame, i.e. frames seen since the stream started.
    pub fn frames_seen(&self) -> u64 {
        self.next_index
    }

    /// Drops any open segment (stop, emergency purge).
    pub fn reset(&mut self) {
        self.frames.clear();
        self.active = false;
        self.above_run = 0;
        self.silence_run = 0;
    }

    /// Feeds one `FRAME`-long frame and its speech probability.
    pub fn push(&mut self, samples: &[f32], prob: f32) -> Option<Outcome> {
        debug_assert_eq!(samples.len(), FRAME);
        let frame = Frame {
            samples: samples.to_vec(),
            prob,
            db: rms_db(samples),
            index: self.next_index,
        };
        self.next_index += 1;

        if !self.active {
            self.frames.push_back(frame);
            while self.frames.len() > self.cfg.pre_roll_frames.max(self.cfg.start_frames) {
                self.frames.pop_front();
            }
            self.above_run = if prob >= self.cfg.threshold {
                self.above_run + 1
            } else {
                0
            };
            if self.above_run >= self.cfg.start_frames {
                self.active = true;
                self.silence_run = 0;
            }
            return None;
        }

        let voiced = prob >= self.cfg.release();
        self.silence_run = if voiced { 0 } else { self.silence_run + 1 };
        self.frames.push_back(frame);

        if self.silence_run >= self.cfg.hang_frames {
            let frames: Vec<Frame> = self.frames.drain(..).collect();
            self.active = false;
            self.above_run = 0;
            return Some(self.validate(frames, false));
        }
        if self.frames.len() >= self.cfg.max_frames {
            // Cut at the quietest frame of the last second, keep the rest open.
            let len = self.frames.len();
            let window = 31.min(len / 2);
            let cut = (len - window..len)
                .min_by(|&a, &b| self.frames[a].prob.total_cmp(&self.frames[b].prob))
                .unwrap_or(len)
                + 1;
            let head: Vec<Frame> = self.frames.drain(..cut).collect();
            self.silence_run = 0;
            return Some(self.validate(head, true));
        }
        None
    }

    fn validate(&self, frames: Vec<Frame>, forced: bool) -> Outcome {
        let release = self.cfg.release();
        let voiced: Vec<&Frame> = frames.iter().filter(|f| f.prob >= release).collect();
        if voiced.len() < self.cfg.min_voiced_frames {
            return Outcome::Rejected(RejectReason::Short);
        }
        if !forced && looks_impulsive(&voiced) {
            return Outcome::Rejected(RejectReason::Transient);
        }
        let mut samples = Vec::with_capacity(frames.len() * FRAME);
        for f in &frames {
            samples.extend_from_slice(&f.samples);
        }
        Outcome::Segment(Segment {
            samples,
            start_frame: frames[0].index,
            last_voiced_frame: voiced.last().map(|f| f.index).unwrap_or(frames[0].index),
            voiced_frames: voiced.len(),
            forced,
        })
    }
}

/// Shots and explosions: one very loud attack then a decaying tail, unlike
/// speech whose syllables keep a fairly even level.
fn looks_impulsive(voiced: &[&Frame]) -> bool {
    let db: Vec<f32> = voiced.iter().map(|f| f.db).collect();
    let (peak_at, peak) = db
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, -90.0));
    let mut sorted = db.clone();
    sorted.sort_by(f32::total_cmp);
    let median = sorted[sorted.len() / 2];
    let spread = peak - median;

    // A short burst dominated by its loudest frame (gunshot + reverb).
    let short = db.len() as u32 * FRAME_MS <= 600;
    if short && spread >= 12.0 {
        return true;
    }
    // A longer sound that peaks at once and only decays (explosion, flashbang).
    if peak_at <= 2 && spread >= 10.0 {
        let tail = &db[peak_at..];
        let slope = slope_db_per_s(tail);
        let rises = tail.windows(2).filter(|w| w[1] > w[0] + 3.0).count();
        return slope <= -15.0 && rises <= tail.len() / 10;
    }
    false
}

/// Least-squares slope of a per-frame level curve, in dB per second.
fn slope_db_per_s(db: &[f32]) -> f32 {
    let n = db.len() as f32;
    if n < 2.0 {
        return 0.0;
    }
    let mean_x = (n - 1.0) / 2.0;
    let mean_y = db.iter().sum::<f32>() / n;
    let (mut num, mut den) = (0.0, 0.0);
    for (i, y) in db.iter().enumerate() {
        let dx = i as f32 - mean_x;
        num += dx * (y - mean_y);
        den += dx * dx;
    }
    num / den * (1000.0 / FRAME_MS as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(amp: f32) -> Vec<f32> {
        (0..FRAME).map(|i| amp * (i as f32 * 0.3).sin()).collect()
    }

    /// Feeds (amplitude, probability) frames; collects outcomes.
    fn run(seg: &mut Segmenter, frames: &[(f32, f32)]) -> Vec<Outcome> {
        frames
            .iter()
            .filter_map(|&(a, p)| seg.push(&frame(a), p))
            .collect()
    }

    fn repeat(n: usize, amp: f32, prob: f32) -> Vec<(f32, f32)> {
        vec![(amp, prob); n]
    }

    fn speech_like(n: usize) -> Vec<(f32, f32)> {
        // Syllables: level varies by a few dB, probability stays high.
        (0..n)
            .map(|i| (0.2 + 0.08 * ((i % 5) as f32 / 4.0), 0.9))
            .collect()
    }

    #[test]
    fn emits_an_utterance_with_pre_and_post_roll() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        let mut input = repeat(20, 0.001, 0.05);
        input.extend(speech_like(30));
        input.extend(repeat(20, 0.001, 0.05));
        let out = run(&mut s, &input);
        assert_eq!(out.len(), 1);
        let Outcome::Segment(seg) = &out[0] else {
            panic!("{out:?}")
        };
        // 8 pre-roll frames (incl. the 2 onset frames) + 28 more speech + 13 post-roll.
        assert_eq!(seg.samples.len(), (8 - 2 + 30 + 13) * FRAME);
        assert_eq!(seg.start_frame, 20 - 6);
        assert_eq!(seg.last_voiced_frame, 49);
        assert_eq!(seg.voiced_frames, 30);
        assert!(!seg.forced);
        assert!(!s.is_speaking());
    }

    #[test]
    fn keeps_short_pauses_inside_the_segment() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        let mut input = speech_like(20);
        input.extend(repeat(8, 0.001, 0.4)); // pause of 256 ms, above release 0.35
        input.extend(repeat(5, 0.001, 0.05)); // and below release, < hang
        input.extend(speech_like(20));
        input.extend(repeat(15, 0.0, 0.0));
        let out = run(&mut s, &input);
        assert_eq!(out.len(), 1, "{out:?}");
    }

    #[test]
    fn ignores_isolated_spikes_and_rejects_short_blips() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        let mut input = Vec::new();
        for _ in 0..5 {
            input.push((0.3, 0.9));
            input.extend(repeat(4, 0.0, 0.0));
        }
        assert!(run(&mut s, &input).is_empty());

        let mut input = speech_like(4);
        input.extend(repeat(20, 0.0, 0.0));
        assert_eq!(
            run(&mut s, &input),
            vec![Outcome::Rejected(RejectReason::Short)]
        );
    }

    #[test]
    fn rejects_a_gunshot_burst() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        // Attack at full scale, then a reverb tail decaying ~6 dB per frame.
        let mut input = vec![(0.0, 0.0); 10];
        let mut amp = 0.9f32;
        for _ in 0..12 {
            input.push((amp, 0.8));
            amp *= 0.5;
        }
        input.extend(repeat(20, 0.0, 0.0));
        assert_eq!(
            run(&mut s, &input),
            vec![Outcome::Rejected(RejectReason::Transient)]
        );
    }

    #[test]
    fn rejects_a_long_decaying_explosion() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        let mut input = Vec::new();
        let mut amp = 0.9f32;
        for _ in 0..40 {
            input.push((amp, 0.7));
            amp *= 0.87; // -1.2 dB per frame, ~ -38 dB/s
        }
        input.extend(repeat(20, 0.0, 0.0));
        assert_eq!(
            run(&mut s, &input),
            vec![Outcome::Rejected(RejectReason::Transient)]
        );
    }

    #[test]
    fn keeps_speech_with_a_shot_on_top() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        let mut input = speech_like(30);
        input[10] = (0.95, 0.9);
        input.extend(repeat(20, 0.0, 0.0));
        let out = run(&mut s, &input);
        assert!(matches!(out.as_slice(), [Outcome::Segment(_)]), "{out:?}");
    }

    #[test]
    fn cuts_long_monologues_at_the_quietest_frame() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        let mut input = speech_like(150);
        input[145] = (0.05, 0.36); // breath, still voiced (release 0.35)
        input.extend(speech_like(40));
        input.extend(repeat(15, 0.0, 0.0));
        let out = run(&mut s, &input);
        assert_eq!(out.len(), 2, "{out:?}");
        let Outcome::Segment(first) = &out[0] else {
            panic!()
        };
        assert!(first.forced);
        assert_eq!(first.samples.len(), 146 * FRAME);
        let Outcome::Segment(second) = &out[1] else {
            panic!()
        };
        assert!(!second.forced);
        assert_eq!(second.start_frame, 146);
    }

    #[test]
    fn exposes_the_open_segment_while_speaking() {
        let mut seg = Segmenter::new(SegmenterConfig::new(0.5));
        assert!(seg.open_segment().is_none());
        run(&mut seg, &speech_like(40));
        assert!(seg.is_speaking());
        let open = seg.open_segment().unwrap();
        assert_eq!(seg.open_frames() * FRAME, open.samples.len());
        assert!(open.voiced_frames >= 30 && open.samples.len() >= 40 * FRAME);
        // The final segment starts like the partial view of it.
        let out = run(&mut seg, &repeat(20, 0.0, 0.0));
        let Some(Outcome::Segment(fin)) = out.last() else {
            panic!("{out:?}")
        };
        assert_eq!(fin.start_frame, open.start_frame);
        assert_eq!(&fin.samples[..open.samples.len()], &open.samples[..]);
        assert_eq!(seg.open_frames(), 0);
    }

    #[test]
    fn reset_drops_the_open_segment() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        run(&mut s, &speech_like(20));
        assert!(s.is_speaking());
        s.reset();
        assert!(!s.is_speaking());
        assert!(run(&mut s, &repeat(20, 0.0, 0.0)).is_empty());
    }

    #[test]
    fn threshold_changes_apply_live() {
        let mut s = Segmenter::new(SegmenterConfig::new(0.5));
        assert!(run(&mut s, &repeat(10, 0.2, 0.6)).is_empty());
        assert!(s.is_speaking());
        s.reset();
        s.set_threshold(0.8);
        run(&mut s, &repeat(10, 0.2, 0.6));
        assert!(!s.is_speaking());
        assert!((s.config().release() - 0.65).abs() < 1e-6);
    }

    #[test]
    fn slope_of_a_decay() {
        let db: Vec<f32> = (0..10).map(|i| -10.0 - i as f32).collect();
        assert!((slope_db_per_s(&db) + 31.25).abs() < 0.01);
        assert_eq!(slope_db_per_s(&[1.0]), 0.0);
    }
}
