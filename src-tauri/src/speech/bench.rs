//! Offline evaluation helpers, shared by `examples/asr_bench.rs` and the tests:
//! the app's exact pipeline run over a file, error rates, and synthetic CS2
//! combat noise.
//!
//! The synthetic noise is a stress test for the shot/explosion guards, not a
//! substitute for real CS2 recordings: those are evaluated with the same tool
//! on the user's own (private) captures.

use std::time::Instant;

use super::dsp::Conditioner;
use super::engine::{RecognizeOptions, Recognizer};
use super::filter::{judge, prompt_from, Recognition};
use super::segmenter::{Outcome, Segmenter, SegmenterConfig};
use super::service::VAD_HOP_FRAMES;
use super::types::{Lang, RejectReason, SpeechConfig};
use super::vad::{VoiceDetector, FRAME, FRAME_MS};
use super::SpeechError;

/// What happened to one segment.
#[derive(Debug, Clone)]
pub struct SegmentResult {
    pub start_ms: u32,
    pub audio_ms: u32,
    pub asr_ms: u32,
    /// Accepted text, or why the segment was dropped.
    pub outcome: Result<String, RejectReason>,
    pub recognition: Option<Recognition>,
}

/// Runs `mono` (any rate) through conditioning, VAD, segmentation, Whisper
/// (when given) and the filter, exactly as the app does.
pub fn run_pipeline(
    mono: &[f32],
    rate: u32,
    detector: &mut dyn VoiceDetector,
    mut recognizer: Option<&mut dyn Recognizer>,
    config: &SpeechConfig,
    threads: usize,
) -> Result<Vec<SegmentResult>, SpeechError> {
    let mut audio = Vec::with_capacity(mono.len() / 2);
    let mut cond = Conditioner::new(rate);
    cond.process(mono, &mut audio);
    // Trailing silence so that a final utterance gets closed.
    let tail = SegmenterConfig::new(config.vad_threshold).hang_frames + 2;
    audio.extend(std::iter::repeat(0.0).take(tail * FRAME));
    audio.truncate(audio.len() / FRAME * FRAME);

    // Same hop as the app's VAD thread (Silero state is rebuilt from context).
    detector.reset();
    let mut probs = Vec::with_capacity(audio.len() / FRAME);
    for chunk in audio.chunks(VAD_HOP_FRAMES * FRAME) {
        probs.extend(detector.probabilities(chunk)?);
    }
    let mut seg = Segmenter::new(SegmenterConfig::new(config.vad_threshold));
    let prompt = prompt_from(&config.glossary);
    let mut out = Vec::new();
    for (frame, prob) in audio.chunks_exact(FRAME).zip(probs) {
        let Some(outcome) = seg.push(frame, prob) else {
            continue;
        };
        let end_ms = (seg.frames_seen() as u32) * FRAME_MS;
        match outcome {
            Outcome::Rejected(r) => out.push(SegmentResult {
                start_ms: end_ms,
                audio_ms: 0,
                asr_ms: 0,
                outcome: Err(r),
                recognition: None,
            }),
            Outcome::Segment(s) => {
                let audio_ms = s.duration_ms();
                let start_ms = s.start_frame as u32 * FRAME_MS;
                let Some(rec) = recognizer.as_deref_mut() else {
                    out.push(SegmentResult {
                        start_ms,
                        audio_ms,
                        asr_ms: 0,
                        outcome: Ok(String::new()),
                        recognition: None,
                    });
                    continue;
                };
                let t0 = Instant::now();
                let r = rec.recognize(
                    &s.samples,
                    &RecognizeOptions {
                        languages: &config.languages,
                        prompt: &prompt,
                        threads,
                    },
                )?;
                out.push(SegmentResult {
                    start_ms,
                    audio_ms,
                    asr_ms: t0.elapsed().as_millis() as u32,
                    outcome: judge(&r, audio_ms),
                    recognition: Some(r),
                });
            }
        }
    }
    Ok(out)
}

/// Lower case, letters and digits only, `ё` folded into `е`.
pub fn normalize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('ё', "е")
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|w| w.trim_matches('\'').to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

fn edit_distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, x) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(x != y);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Word errors and reference length (WER = errors / words).
pub fn word_errors(reference: &str, hypothesis: &str) -> (usize, usize) {
    let r = normalize(reference);
    let h = normalize(hypothesis);
    (edit_distance(&r, &h), r.len())
}

/// Character errors (spaces ignored) and reference length.
pub fn char_errors(reference: &str, hypothesis: &str) -> (usize, usize) {
    let r: Vec<char> = normalize(reference).concat().chars().collect();
    let h: Vec<char> = normalize(hypothesis).concat().chars().collect();
    (edit_distance(&r, &h), r.len())
}

/// Deterministic generator for reproducible noise.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    pub fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Uniform in -1..1.
    pub fn noise(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

/// Kind of synthetic game sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseEvent {
    /// Single rifle shot: broadband crack, fast decay, short reverb tail.
    Shot,
    /// Automatic fire: 3 to 8 shots at ~600 rounds per minute.
    Burst,
    /// HE grenade: crack plus long low-frequency rumble.
    Explosion,
}

/// Renders one event at `rate`, peak normalised to 1.
pub fn render_event(kind: NoiseEvent, rate: u32, rng: &mut Rng) -> Vec<f32> {
    let r = rate as f32;
    let shot = |rng: &mut Rng, out: &mut Vec<f32>, at: usize| {
        let len = (0.35 * r) as usize;
        if out.len() < at + len {
            out.resize(at + len, 0.0);
        }
        let mut lp = 0.0f32;
        for i in 0..len {
            let t = i as f32 / r;
            let crack = (-t / 0.012).exp();
            let tail = 0.25 * (-t / 0.12).exp();
            let n = rng.noise();
            lp += 0.35 * (n - lp); // a bit of low-pass for the tail
            out[at + i] += n * crack + lp * tail;
        }
    };
    let mut out = Vec::new();
    match kind {
        NoiseEvent::Shot => shot(rng, &mut out, 0),
        NoiseEvent::Burst => {
            let n = 3 + (rng.next_f32() * 6.0) as usize;
            for k in 0..n {
                shot(rng, &mut out, (k as f32 * 0.1 * r) as usize);
            }
        }
        NoiseEvent::Explosion => {
            let len = (1.6 * r) as usize;
            out.resize(len, 0.0);
            let (mut lp1, mut lp2) = (0.0f32, 0.0f32);
            let a = 1.0 - (-2.0 * std::f32::consts::PI * 180.0 / r).exp();
            for (i, s) in out.iter_mut().enumerate() {
                let t = i as f32 / r;
                let n = rng.noise();
                lp1 += a * (n - lp1);
                lp2 += a * (lp1 - lp2);
                let rumble = 6.0 * lp2 * (-t / 0.45).exp();
                let crack = n * (-t / 0.02).exp();
                *s = rumble + crack;
            }
        }
    }
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    out.iter_mut().for_each(|s| *s /= peak);
    out
}

/// Synthetic combat track: bursts every 1–3 s, an explosion every ~7 s and a
/// low ambient bed. `peak` is the linear peak level of the events.
pub fn combat_track(len: usize, rate: u32, peak: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut out: Vec<f32> = (0..len).map(|_| rng.noise() * 0.004).collect();
    let mut t = (0.3 * rate as f32) as usize;
    let mut since_explosion = 0.0f32;
    while t < len {
        let kind = if since_explosion > 6.0 && rng.next_f32() < 0.5 {
            since_explosion = 0.0;
            NoiseEvent::Explosion
        } else if rng.next_f32() < 0.3 {
            NoiseEvent::Shot
        } else {
            NoiseEvent::Burst
        };
        let ev = render_event(kind, rate, &mut rng);
        let gain = peak * (0.5 + 0.5 * rng.next_f32());
        for (i, s) in ev.iter().enumerate() {
            if let Some(o) = out.get_mut(t + i) {
                *o += s * gain;
            }
        }
        let gap = 1.0 + 2.0 * rng.next_f32();
        since_explosion += gap;
        t += (gap * rate as f32) as usize;
    }
    out
}

/// Adds `noise` to `speech` (in place), with soft clipping at full scale.
pub fn mix(speech: &mut [f32], noise: &[f32]) {
    for (s, n) in speech.iter_mut().zip(noise) {
        *s = (*s + n).clamp(-1.0, 1.0);
    }
}

/// Peak level in dBFS.
pub fn peak_db(x: &[f32]) -> f32 {
    let p = x.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    (20.0 * p.max(1e-9).log10()).max(-90.0)
}

/// Decoded WAV file: mono mix and sample rate.
pub struct Wav {
    pub mono: Vec<f32>,
    pub rate: u32,
}

/// Reads a PCM (16/24/32-bit) or float32 WAV file, any channel count.
pub fn read_wav(bytes: &[u8]) -> Result<Wav, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("pas un fichier WAV".into());
    }
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at =
        |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let mut pos = 12;
    let mut fmt = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32_at(pos + 4) as usize;
        let body = pos + 8;
        let end = (body + len).min(bytes.len());
        if id == b"fmt " && len >= 16 {
            let mut tag = u16_at(body);
            if tag == 0xFFFE && len >= 26 {
                tag = u16_at(body + 24); // sub-format GUID starts with the format tag
            }
            fmt = Some((tag, u16_at(body + 2), u32_at(body + 4), u16_at(body + 14)));
        } else if id == b"data" {
            let (tag, channels, rate, bits) = fmt.ok_or("bloc fmt manquant")?;
            let format = match (tag, bits) {
                (1, 16) => crate::audio::SampleFormat::I16,
                (1, 24) => crate::audio::SampleFormat::I24,
                (1, 32) => crate::audio::SampleFormat::I32,
                (3, 32) => crate::audio::SampleFormat::F32,
                _ => return Err(format!("format WAV non géré ({tag}, {bits} bits)")),
            };
            let mut interleaved = Vec::new();
            crate::audio::convert::to_f32(&bytes[body..end], format, &mut interleaved);
            let mono = crate::audio::convert::mono_frames(&interleaved, channels).collect();
            return Ok(Wav { mono, rate });
        }
        pos = body + len + (len & 1);
    }
    Err("bloc data manquant".into())
}

/// 16-bit mono WAV, for tests and for exporting mixed bench files.
pub fn write_wav_i16(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

/// Languages accepted on the command line ("ru,en").
pub fn parse_langs(s: &str) -> Vec<Lang> {
    s.split(',')
        .filter_map(|c| Lang::from_code(c.trim()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::vad::EnergyDetector;

    #[test]
    fn error_rates() {
        assert_eq!(word_errors("Двое идут на B", "двое идут на б"), (1, 4));
        assert_eq!(
            word_errors("one low, he's lit", "One low. He's lit!"),
            (0, 4)
        );
        assert_eq!(word_errors("a b c", ""), (3, 3));
        assert_eq!(word_errors("", "x"), (1, 0));
        assert_eq!(char_errors("ёлка", "елка"), (0, 4));
        assert_eq!(char_errors("abc", "abd"), (1, 3));
    }

    #[test]
    fn wav_round_trip() {
        let x: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let wav = read_wav(&write_wav_i16(&x, 22_050)).unwrap();
        assert_eq!(wav.rate, 22_050);
        assert_eq!(wav.mono.len(), 1000);
        assert!(wav.mono.iter().zip(&x).all(|(a, b)| (a - b).abs() < 1e-4));
        assert!(read_wav(b"nope").is_err());
    }

    #[test]
    fn noise_is_reproducible_and_bounded() {
        let a = combat_track(48_000 * 5, 48_000, 0.8, 7);
        let b = combat_track(48_000 * 5, 48_000, 0.8, 7);
        assert_eq!(a, b);
        assert!(peak_db(&a) <= 0.0);
        assert!(peak_db(&a) > -6.0, "events present: {}", peak_db(&a));
        for kind in [NoiseEvent::Shot, NoiseEvent::Burst, NoiseEvent::Explosion] {
            let e = render_event(kind, 16_000, &mut Rng::new(3));
            assert!((peak_db(&e)).abs() < 1e-3);
        }
    }

    #[test]
    fn pipeline_rejects_synthetic_shots_with_an_energy_vad() {
        // Worst case for the guards: a VAD that calls every loud frame speech.
        let rate = 16_000;
        let mut det = EnergyDetector {
            threshold_db: -35.0,
        };
        let mut rng = Rng::new(11);
        let mut track = vec![0.0f32; rate as usize];
        for kind in [NoiseEvent::Shot, NoiseEvent::Explosion] {
            track.extend(render_event(kind, rate, &mut rng).iter().map(|s| s * 0.9));
            track.extend(vec![0.0f32; rate as usize]);
        }
        let res = run_pipeline(&track, rate, &mut det, None, &SpeechConfig::default(), 1).unwrap();
        assert!(!res.is_empty());
        assert!(
            res.iter().all(|r| r.outcome.is_err()),
            "every noise segment is rejected before Whisper: {res:?}"
        );
    }

    #[test]
    fn pipeline_keeps_a_steady_voice_like_signal() {
        let rate = 48_000;
        let mut det = EnergyDetector {
            threshold_db: -35.0,
        };
        let mut x = vec![0.0f32; rate as usize / 2];
        x.extend(
            (0..rate as usize)
                .map(|i| 0.3 * (i as f32 * 0.05).sin() * (1.0 + 0.2 * (i as f32 * 0.0005).sin())),
        );
        x.extend(vec![0.0f32; rate as usize / 2]);
        let res = run_pipeline(&x, rate, &mut det, None, &SpeechConfig::default(), 1).unwrap();
        assert_eq!(res.len(), 1, "{res:?}");
        assert!(res[0].outcome.is_ok());
        assert!(
            (900..1700).contains(&res[0].audio_ms),
            "{}",
            res[0].audio_ms
        );
        assert_eq!(parse_langs("ru, en,xx"), vec![Lang::Ru, Lang::En]);
    }
}
