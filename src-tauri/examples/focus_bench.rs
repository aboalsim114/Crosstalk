//! Measures "Focus voix" (src/speech/focus.rs) on a real recorded voice mixed
//! with synthetic, spatialised CS2 combat noise:
//!
//! crosstalk-focus-bench --voice corpus/jfk.wav --vad models/ggml-silero-v5.1.2.bin
//!
//! The voice is put in the centre (as CS2's voice chat is assumed to be);
//! shots, bursts and explosions come from several directions (level and delay
//! differences between the ears). Prints the signal-to-noise ratio of the
//! centre with and without the focus, and what the VAD makes of each.
//! The noise is a stress test, not a real CS2 recording: real captures are
//! needed to confirm the gain (docs/testing/voice-focus.md).

use std::path::Path;

use crosstalk_lib::speech::bench::{combat_track, read_wav, run_pipeline, Rng};
use crosstalk_lib::speech::focus::{snr_db, VoiceFocus, LATENCY_FRAMES};
use crosstalk_lib::speech::types::SpeechConfig;
use crosstalk_lib::speech::vad::SileroVad;

const RATE: u32 = 48_000;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// Linear interpolation to 48 kHz (the capture rate).
fn to_48k(x: &[f32], rate: u32) -> Vec<f32> {
    let n = (x.len() as u64 * RATE as u64 / rate as u64) as usize;
    (0..n)
        .map(|i| {
            let p = i as f64 * rate as f64 / RATE as f64;
            let k = p as usize;
            let f = (p - k as f64) as f32;
            let a = x.get(k).copied().unwrap_or(0.0);
            let b = x.get(k + 1).copied().unwrap_or(a);
            a + (b - a) * f
        })
        .collect()
}

/// A game sound source: gains left / right and delay of the far ear (frames;
/// positive: right ear later).
struct Source {
    gl: f32,
    gr: f32,
    itd: i32,
    seed: u64,
}

fn place(track: &[f32], s: &Source, st: &mut [f32]) {
    let n = st.len() / 2;
    for i in 0..n {
        let li = i as i32 - (-s.itd).max(0);
        let ri = i as i32 - s.itd.max(0);
        let at = |k: i32| {
            if k >= 0 {
                track.get(k as usize).copied().unwrap_or(0.0)
            } else {
                0.0
            }
        };
        st[2 * i] += s.gl * at(li);
        st[2 * i + 1] += s.gr * at(ri);
    }
}

struct Vad {
    segments: usize,
    on_voice: usize,
    false_alarms: usize,
    speech_ms: u32,
}

fn vad(mono: &[f32], det: &mut SileroVad, voice_spans: &[(u32, u32)]) -> Vad {
    let cfg = SpeechConfig::default();
    let segs = run_pipeline(mono, RATE, det, None, &cfg).expect("pipeline");
    let segs: Vec<_> = segs.into_iter().filter(|s| s.audio_ms > 0).collect();
    let overlaps = |s: u32, e: u32| voice_spans.iter().any(|&(a, b)| s < b && a < e);
    let on_voice = segs
        .iter()
        .filter(|s| overlaps(s.start_ms, s.start_ms + s.audio_ms))
        .count();
    Vad {
        segments: segs.len(),
        on_voice,
        false_alarms: segs.len() - on_voice,
        speech_ms: segs.iter().map(|s| s.audio_ms).sum(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let voice_path = arg(&args, "--voice").unwrap_or_else(|| "corpus/jfk.wav".into());
    let vad_path = arg(&args, "--vad");
    let peak: f32 = arg(&args, "--peak")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.5);
    let wav = read_wav(&std::fs::read(&voice_path).expect("voice file")).expect("wav");
    let voice = to_48k(&wav.mono, wav.rate);

    // Voice three times, with gaps of combat only.
    let gap = (4.0 * RATE as f32) as usize;
    let len = 3 * (voice.len() + gap) + gap;
    let mut clean = vec![0.0f32; len];
    let mut spans = Vec::new();
    for k in 0..3 {
        let at = gap + k * (voice.len() + gap);
        clean[at..at + voice.len()].copy_from_slice(&voice);
        let ms = |f: usize| (f as u64 * 1000 / RATE as u64) as u32;
        spans.push((ms(at), ms(at + voice.len())));
    }

    let scenarios: [(&str, Vec<Source>); 3] = [
        (
            "combat sur les côtés",
            vec![
                Source {
                    gl: 0.9,
                    gr: 0.35,
                    itd: 30,
                    seed: 11,
                },
                Source {
                    gl: 0.3,
                    gr: 0.85,
                    itd: -26,
                    seed: 23,
                },
            ],
        ),
        (
            "combat varié (côtés + presque devant)",
            vec![
                Source {
                    gl: 0.9,
                    gr: 0.35,
                    itd: 30,
                    seed: 11,
                },
                Source {
                    gl: 0.3,
                    gr: 0.85,
                    itd: -26,
                    seed: 23,
                },
                Source {
                    gl: 0.7,
                    gr: 0.6,
                    itd: 6,
                    seed: 37,
                },
            ],
        ),
        (
            "avec ses propres tirs (au centre)",
            vec![
                Source {
                    gl: 0.9,
                    gr: 0.35,
                    itd: 30,
                    seed: 11,
                },
                Source {
                    gl: 0.3,
                    gr: 0.85,
                    itd: -26,
                    seed: 23,
                },
                Source {
                    gl: 0.5,
                    gr: 0.5,
                    itd: 0,
                    seed: 41,
                },
            ],
        ),
    ];

    let mut det = vad_path
        .as_deref()
        .map(|p| SileroVad::load(Path::new(p), 0.5).expect("VAD"));
    println!("Voix : {voice_path} (x3, au centre) · bruit de combat synthétique, crête {peak}");
    println!("| Scénario | RSB sans focus | RSB avec focus | Gain | VAD sans (segments sur voix / faux) | VAD avec |");
    println!("|---|---|---|---|---|---|");
    let mut costs = Vec::new();
    for (name, sources) in &scenarios {
        let mut st: Vec<f32> = clean.iter().flat_map(|&v| [v, v]).collect();
        let mut rng = Rng::new(5);
        for s in sources {
            let track = combat_track(len, RATE, peak * (0.8 + 0.4 * rng.next_f32()), s.seed);
            place(&track, s, &mut st);
        }
        let plain: Vec<f32> = st.chunks(2).map(|f| 0.5 * (f[0] + f[1])).collect();
        let mut focus = VoiceFocus::default();
        let mut out = Vec::new();
        let t0 = std::time::Instant::now();
        for block in st.chunks(960 * 2) {
            focus.process(block, 2, &mut out);
        }
        let cost = t0.elapsed().as_secs_f64() / (len as f64 / RATE as f64) * 100.0;
        costs.push(cost);
        let out: Vec<f32> = out[LATENCY_FRAMES..].to_vec();
        // SNR on the voice parts only (what Whisper hears).
        let (mut c, mut p, mut o) = (Vec::new(), Vec::new(), Vec::new());
        for &(a, b) in &spans {
            let (a, b) = ((a as usize) * 48, (b as usize) * 48);
            c.extend_from_slice(&clean[a..b]);
            p.extend_from_slice(&plain[a..b]);
            o.extend_from_slice(&out[a..b.min(out.len())]);
        }
        let before = snr_db(&c, &p, 0);
        let after = snr_db(&c, &o, 0);
        let vads = det
            .as_mut()
            .map(|d| (vad(&plain, d, &spans), vad(&out, d, &spans)));
        let fmt = |v: &Vad| format!("{} / {} ({} ms)", v.on_voice, v.false_alarms, v.speech_ms);
        let (vb, va) = vads
            .as_ref()
            .map(|(b, a)| (fmt(b), fmt(a)))
            .unwrap_or_else(|| ("—".into(), "—".into()));
        let _ = vads.as_ref().map(|(b, _)| b.segments);
        println!(
            "| {name} | {before:.1} dB | {after:.1} dB | {:+.1} dB | {vb} | {va} |",
            after - before
        );
    }
    let avg = costs.iter().sum::<f64>() / costs.len().max(1) as f64;
    println!("\nCoût du focus : {avg:.2} % d'un cœur CPU (temps de calcul / durée audio, 48 kHz stéréo).");
}
