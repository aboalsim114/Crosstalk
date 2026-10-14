//! Offline evaluation helpers, shared by `examples/asr_bench.rs` and the tests:
//! the app's exact pipeline run over a file, error rates, and synthetic CS2
//! combat noise.
//!
//! The synthetic noise is a stress test for the shot/explosion guards, not a
//! substitute for real CS2 recordings: those are evaluated with the same tool
//! on the user's own (private) captures.

use std::time::Instant;

use super::dsp::Conditioner;
use super::engine::{InferenceTiming, RecognizeOptions, Recognizer};
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
    pub timing: Option<InferenceTiming>,
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
                timing: None,
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
                        timing: None,
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
                        fast_encoder: config.fast_encoder,
                    },
                )?;
                out.push(SegmentResult {
                    start_ms,
                    audio_ms,
                    asr_ms: t0.elapsed().as_millis() as u32,
                    outcome: judge(&r, audio_ms),
                    recognition: Some(r),
                    timing: rec.timing(),
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

/// Linear interpolation to `to` Hz (bench only: voice files are often 16 kHz
/// while the game is captured at 48 kHz).
pub fn resample_linear(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return x.to_vec();
    }
    let n = (x.len() as u64 * to as u64 / from.max(1) as u64) as usize;
    (0..n)
        .map(|i| {
            let p = i as f64 * from as f64 / to as f64;
            let k = p as usize;
            let f = (p - k as f64) as f32;
            let a = x.get(k).copied().unwrap_or(0.0);
            let b = x.get(k + 1).copied().unwrap_or(a);
            a + (b - a) * f
        })
        .collect()
}

/// Rate of the stereo combat scenes (CS2's mix format: 48 kHz float stereo).
pub const SCENE_RATE: u32 = 48_000;
/// Combat without voice before and after each line (where a subtitle would
/// be a false positive).
pub const SCENE_GAP_S: f32 = 3.0;

/// A game sound source: gains left / right and delay of the far ear
/// (frames; positive: the right ear hears it later).
struct Source {
    gl: f32,
    gr: f32,
    itd: i32,
    seed: u64,
}

/// A voice line in a synthetic, spatialised CS2 scene (48 kHz stereo).
///
/// After the REA investigation of CS2's audio graphs: the voice chat (VOIP
/// sub-mix) is mono, not spatialised, added to the game bus at the very end,
/// so it is put in the centre; game sounds come from the sides (level and
/// delay between the ears), and with `own_shots` also from the centre.
pub struct Scene {
    pub stereo: Vec<f32>,
    /// The voice alone, on the same timeline (to measure what reaches Whisper).
    pub clean: Vec<f32>,
    /// Where the voice is, in ms.
    pub voice_ms: (u32, u32),
}

/// CS2's voice chain high-pass (REA report §8.3, decoded `csgo_voip.vmix_c`:
/// 207.59 Hz, 48 dB/oct): 8th-order Butterworth, four biquads.
pub fn cs2_voip_highpass(x: &mut [f32], rate: u32) {
    for q in [0.509_795_6, 0.601_344_9, 0.899_976_1, 2.562_915_4] {
        super::dsp::HighPass::with_q(207.59, rate, q).process(x);
    }
}

pub fn combat_scene(voice: &[f32], rate: u32, peak: f32, seed: u64, own_shots: bool) -> Scene {
    let mut v = resample_linear(voice, rate, SCENE_RATE);
    // The voice as CS2 plays it: no content below ~200 Hz.
    cs2_voip_highpass(&mut v, SCENE_RATE);
    let gain = 10f32.powf((-6.0 - peak_db(&v)) / 20.0);
    v.iter_mut().for_each(|s| *s *= gain);
    let gap = (SCENE_GAP_S * SCENE_RATE as f32) as usize;
    let len = gap + v.len() + gap;
    let mut st = vec![0.0f32; len * 2];
    let mut clean = vec![0.0f32; len];
    for (i, s) in v.iter().enumerate() {
        st[2 * (gap + i)] = *s;
        st[2 * (gap + i) + 1] = *s;
        clean[gap + i] = *s;
    }
    let mut sources = vec![
        Source {
            gl: 0.9,
            gr: 0.35,
            itd: 30,
            seed,
        },
        Source {
            gl: 0.3,
            gr: 0.85,
            itd: -26,
            seed: seed + 1,
        },
        Source {
            gl: 0.7,
            gr: 0.6,
            itd: 6,
            seed: seed + 2,
        },
    ];
    if own_shots {
        sources.push(Source {
            gl: 0.5,
            gr: 0.5,
            itd: 0,
            seed: seed + 3,
        });
    }
    let mut rng = Rng::new(seed);
    for s in &sources {
        let track = combat_track(len, SCENE_RATE, peak * (0.8 + 0.4 * rng.next_f32()), s.seed);
        let at = |k: i64| {
            if k >= 0 {
                track.get(k as usize).copied().unwrap_or(0.0)
            } else {
                0.0
            }
        };
        for i in 0..len {
            st[2 * i] += s.gl * at(i as i64 - (-s.itd).max(0) as i64);
            st[2 * i + 1] += s.gr * at(i as i64 - s.itd.max(0) as i64);
        }
    }
    st.iter_mut().for_each(|s| *s = s.clamp(-1.0, 1.0));
    let ms = |f: usize| (f as u64 * 1000 / SCENE_RATE as u64) as u32;
    Scene {
        stereo: st,
        clean,
        voice_ms: (ms(gap), ms(gap + v.len())),
    }
}

/// Audio front-ends compared by the bench, before the app's own 16 kHz
/// conditioning, VAD and Whisper (which are the same for all).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Front {
    /// A: current production default (stereo averaged to mono).
    Plain,
    /// B: "Focus voix" (src/speech/focus.rs).
    Focus,
    /// Voice band: CS2 cuts its voice chat below 207.6 Hz (48 dB/oct, REA
    /// report §8.3): below ~200 Hz there is only game sound. 4th-order
    /// Butterworth high-pass at 200 Hz.
    VoiceBand,
    /// B + voice band.
    FocusVoiceBand,
}

impl Front {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "plain" | "a" => Some(Front::Plain),
            "focus" | "b" => Some(Front::Focus),
            "band" => Some(Front::VoiceBand),
            "focus+band" => Some(Front::FocusVoiceBand),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Front::Plain => "A · actuel",
            Front::Focus => "B · Focus voix",
            Front::VoiceBand => "coupure 200 Hz",
            Front::FocusVoiceBand => "B + coupure 200 Hz",
        }
    }
}

/// High-pass at `hz`, 4th-order Butterworth (two biquads).
pub fn voice_band(x: &mut [f32], rate: u32, hz: f32) {
    for q in [0.541_196_1, 1.306_563] {
        super::dsp::HighPass::with_q(hz, rate, q).process(x);
    }
}

/// Mono signal at `rate` for the pipeline, from interleaved stereo.
pub fn apply_front(stereo: &[f32], rate: u32, front: Front) -> Vec<f32> {
    let focus = |st: &[f32]| {
        let mut f = super::focus::VoiceFocus::default();
        let mut out = Vec::with_capacity(st.len() / 2);
        f.process(st, 2, &mut out);
        // Same timeline as the input (the app's 10.7 ms delay is not a
        // quality matter): drop the delay, pad the unflushed end.
        let mut out = out.split_off(super::focus::LATENCY_FRAMES.min(out.len()));
        out.resize(st.len() / 2, 0.0);
        out
    };
    let plain = |st: &[f32]| crate::audio::convert::mono_frames(st, 2).collect::<Vec<f32>>();
    match front {
        Front::Plain => plain(stereo),
        Front::Focus => focus(stereo),
        Front::VoiceBand => {
            let mut m = plain(stereo);
            voice_band(&mut m, rate, 200.0);
            m
        }
        Front::FocusVoiceBand => {
            let mut m = focus(stereo);
            voice_band(&mut m, rate, 200.0);
            m
        }
    }
}

/// What one scene gave, for the A/B comparison.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneScore {
    /// Segments overlapping the voice that reached the user (accepted
    /// transcript; without Whisper: sent to Whisper).
    pub on_voice: usize,
    /// Same, entirely in the combat-only parts: a subtitle that should not
    /// exist (without Whisper: Whisper work for nothing).
    pub false_alarms: usize,
    /// The voice line produced nothing.
    pub missed: bool,
    /// Accepted text (all of it: false positives count as insertions).
    pub text: String,
    /// Share of the voice line inside segments that reached the user (a cut
    /// sentence is a wrong transcript).
    pub coverage: f32,
}

pub fn score_scene(
    results: &[SegmentResult],
    voice_ms: (u32, u32),
    with_whisper: bool,
) -> SceneScore {
    let mut s = SceneScore::default();
    let mut texts = Vec::new();
    let mut covered = 0u32;
    for r in results {
        let shown = match &r.outcome {
            Ok(t) => !with_whisper || !t.trim().is_empty(),
            Err(_) => false,
        };
        if !shown || r.audio_ms == 0 {
            continue;
        }
        let (a, b) = (r.start_ms, r.start_ms + r.audio_ms);
        if a < voice_ms.1 && voice_ms.0 < b {
            s.on_voice += 1;
            covered += b.min(voice_ms.1) - a.max(voice_ms.0);
        } else {
            s.false_alarms += 1;
        }
        if let Ok(t) = &r.outcome {
            texts.push(t.clone());
        }
    }
    s.missed = s.on_voice == 0;
    s.text = texts.join(" ");
    s.coverage = covered as f32 / (voice_ms.1 - voice_ms.0).max(1) as f32;
    s
}

/// Signal-to-noise ratio of the voice part of `mono` against the clean voice
/// (dB): a proxy for what Whisper hears, not a transcription score. The
/// linear part of the front (the voice band filter) is applied to the
/// reference too: only noise and non-linear damage count.
pub fn voice_snr_db(
    clean: &[f32],
    mono: &[f32],
    voice_ms: (u32, u32),
    rate: u32,
    front: Front,
) -> f32 {
    let mut reference = clean.to_vec();
    if matches!(front, Front::VoiceBand | Front::FocusVoiceBand) {
        voice_band(&mut reference, rate, 200.0);
    }
    let at = |ms: u32| (ms as u64 * rate as u64 / 1000) as usize;
    let (a, b) = (
        at(voice_ms.0),
        at(voice_ms.1).min(reference.len()).min(mono.len()),
    );
    super::focus::snr_db(&reference[a..b], &mono[a..b], 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_scenes_put_the_voice_in_the_centre_between_combat_gaps() {
        let voice: Vec<f32> = (0..16_000).map(|i| 0.4 * (i as f32 * 0.05).sin()).collect();
        let sc = combat_scene(&voice, 16_000, 0.5, 9, false);
        assert_eq!(sc.voice_ms, (3000, 4000));
        assert_eq!(sc.clean.len(), 48_000 * 7);
        assert!(voice_snr_db(&sc.clean, &sc.clean, sc.voice_ms, SCENE_RATE, Front::Plain) > 100.0);
        // The CS2 voice high-pass leaves nothing at 100 Hz.
        let mut low: Vec<f32> = (0..48_000)
            .map(|i| (2.0 * std::f32::consts::PI * 100.0 * i as f32 / 48_000.0).sin() * 0.5)
            .collect();
        cs2_voip_highpass(&mut low, 48_000);
        assert!(peak_db(&low[24_000..]) < -6.0 - 45.0);
        assert_eq!(sc.stereo.len(), 2 * 48_000 * 7);
        // Spatialised combat in the gaps (left and right differ), a mono
        // voice in the middle part only where the combat is quiet.
        let gap = &sc.stereo[..2 * 48_000 * 3];
        let (mid, side): (f32, f32) = gap.chunks(2).fold((0.0, 0.0), |a, f| {
            (a.0 + (f[0] + f[1]).powi(2), a.1 + (f[0] - f[1]).powi(2))
        });
        assert!(mid > 0.0 && side > 0.05 * mid, "mid {mid} side {side}");
        for f in [
            Front::Plain,
            Front::Focus,
            Front::VoiceBand,
            Front::FocusVoiceBand,
        ] {
            assert_eq!(
                apply_front(&sc.stereo, SCENE_RATE, f).len(),
                48_000 * 7,
                "{f:?}"
            );
            assert_eq!(
                Front::parse(match f {
                    Front::Plain => "plain",
                    Front::Focus => "focus",
                    Front::VoiceBand => "band",
                    Front::FocusVoiceBand => "focus+band",
                }),
                Some(f)
            );
        }
    }

    #[test]
    fn the_voice_band_removes_rumble_and_keeps_the_voice_range() {
        let tone = |hz: f32| {
            (0..48_000)
                .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / 48_000.0).sin() * 0.5)
                .collect::<Vec<f32>>()
        };
        let mut low = tone(60.0);
        voice_band(&mut low, 48_000, 200.0);
        let mut mid = tone(800.0);
        voice_band(&mut mid, 48_000, 200.0);
        assert!(
            peak_db(&low[24_000..]) < -6.0 - 40.0,
            "60 Hz rumble down > 40 dB"
        );
        assert!(peak_db(&mid[24_000..]) > -6.5, "800 Hz voice kept");
    }

    #[test]
    fn scene_scores_count_misses_and_false_alarms() {
        let seg = |start_ms: u32, audio_ms: u32, text: &str| SegmentResult {
            start_ms,
            audio_ms,
            asr_ms: 1,
            outcome: Ok(text.into()),
            recognition: None,
            timing: None,
        };
        let res = vec![
            seg(500, 400, "bruit"),
            seg(2900, 1000, "ask not"),
            seg(6000, 300, ""),
        ];
        let s = score_scene(&res, (3000, 4000), true);
        assert_eq!((s.on_voice, s.false_alarms, s.missed), (1, 1, false));
        assert!((s.coverage - 0.9).abs() < 1e-6, "{}", s.coverage);
        assert_eq!(s.text, "bruit ask not");
        let s = score_scene(&res, (3000, 4000), false);
        assert_eq!(s.false_alarms, 2, "without Whisper every segment counts");
        assert!(score_scene(&[], (3000, 4000), true).missed);
    }
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
