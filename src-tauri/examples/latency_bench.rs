//! Latency of the real speech pipeline, stage by stage: the clips of a
//! manifest are played in real time, one after the other with a pause,
//! through the app's own speech service (VAD, segmenter, queue, live lines,
//! recogniser), as the capture would feed it.
//!
//! ```text
//! cargo run --release --example latency_bench -- --manifest corpus/manifest.tsv
//! ```
//!
//! Options: `--models-dir DIR`, `--threads N`, `--gap-ms 1200` (pause between
//! clips), `--limit N` (clips), `--load N` (N busy threads at normal priority
//! during the run: a stand-in for CS2), `--lang ru,en`.
//! Manifest: `path<TAB>lang<TAB>reference` (paths relative to the manifest).
//! Nothing is recorded; the clips are only read.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crosstalk_lib::audio::{AudioBlock, BlockSink};
use crosstalk_lib::speech::bench::{parse_langs, read_wav};
use crosstalk_lib::speech::engine::default_threads;
use crosstalk_lib::speech::{
    models, EngineState, RejectReason, SherpaLoader, SpeechConfig, SpeechEvents, SpeechService,
    SpeechStats, SpeechStatus, Transcript, VoiceActivity,
};

#[derive(Default)]
struct Collected {
    finals: Vec<Transcript>,
    partials: usize,
    stats: Option<SpeechStats>,
}

struct Collector(Mutex<Collected>);

impl SpeechEvents for Collector {
    fn status(&self, _s: &SpeechStatus) {}
    fn transcript(&self, t: &Transcript) {
        let mut c = self.0.lock().unwrap();
        if t.partial {
            c.partials += 1;
        } else {
            c.finals.push(t.clone());
        }
    }
    fn retract(&self, _id: u64) {}
    fn activity(&self, _a: VoiceActivity) {}
    fn stats(&self, s: &SpeechStats) {
        self.0.lock().unwrap().stats = Some(s.clone());
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn fail(msg: &str) -> ! {
    eprintln!("erreur : {msg}");
    std::process::exit(1)
}

/// p50 / p95 of a list of ms values.
fn pct(values: &mut [u32]) -> (u32, u32) {
    if values.is_empty() {
        return (0, 0);
    }
    values.sort_unstable();
    let at = |q: f32| values[((q * values.len() as f32) as usize).min(values.len() - 1)];
    (at(0.5), at(0.95))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let manifest = arg(&args, "--manifest").unwrap_or_else(|| fail("--manifest requis"));
    let dir = arg(&args, "--models-dir")
        .map(PathBuf::from)
        .unwrap_or_else(crosstalk_lib::paths::models_dir);
    let threads = arg(&args, "--threads")
        .and_then(|t| t.parse().ok())
        .unwrap_or_else(default_threads);
    let gap_ms: u64 = arg(&args, "--gap-ms")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1200);
    let limit: usize = arg(&args, "--limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(usize::MAX);
    let load: usize = arg(&args, "--load")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut config = SpeechConfig {
        model: models::DEFAULT_MODEL.into(),
        ..SpeechConfig::default()
    };
    if let Some(l) = arg(&args, "--lang") {
        config.languages = parse_langs(&l);
    }

    // The clips, decoded up front: reading files must not delay the stream.
    let base = Path::new(&manifest)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let text = std::fs::read_to_string(&manifest).unwrap_or_else(|e| fail(&e.to_string()));
    let mut clips: Vec<Vec<f32>> = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("id\t"))
    {
        if clips.len() >= limit {
            break;
        }
        let path = base.join(line.split('\t').next().unwrap_or_default());
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        let wav = read_wav(&bytes).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
        if wav.rate != 16_000 {
            fail(&format!("{} : 16 kHz attendu", path.display()));
        }
        clips.push(wav.mono);
    }
    if clips.is_empty() {
        fail("aucun clip");
    }

    let collector = Arc::new(Collector(Mutex::new(Collected::default())));
    let speech = SpeechService::start_with_threads(
        collector.clone(),
        Arc::new(SherpaLoader),
        dir,
        config,
        threads,
    );
    let end = Instant::now() + Duration::from_secs(120);
    while speech.status().state == EngineState::Loading && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    if speech.status().state != EngineState::Ready {
        fail(&format!("moteur indisponible : {:?}", speech.status()));
    }

    // Busy threads at normal priority, like a game using the processor.
    let stop = Arc::new(AtomicBool::new(false));
    let busy: Vec<_> = (0..load)
        .map(|_| {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut x = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    for i in 0..10_000u64 {
                        x = black_box(x.wrapping_mul(6364136223846793005).wrapping_add(i));
                    }
                }
                black_box(x);
            })
        })
        .collect();

    // Real-time stream: 10 ms blocks, a pause of faint noise between clips.
    let mut sink = speech.sink();
    let block = 160usize;
    let mut stream: Vec<f32> = Vec::new();
    let mut seed = 7u32;
    let gap = (gap_ms as usize) * 16;
    for c in &clips {
        stream.extend_from_slice(c);
        stream.extend((0..gap).map(|_| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((seed >> 16) as f32 / 32_768.0 - 1.0) * 0.001
        }));
    }
    println!(
        "{} clips, {:.0} s de flux, {threads} fil(s), charge {load} fil(s), pause {gap_ms} ms",
        clips.len(),
        stream.len() as f64 / 16_000.0
    );
    let t0 = Instant::now();
    for (i, chunk) in stream.chunks(block).enumerate() {
        let due = t0 + Duration::from_millis(10 * i as u64);
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        sink.block(&AudioBlock {
            samples: chunk,
            channels: 1,
            sample_rate: 16_000,
            qpc_100ns: 0,
        });
    }
    // Let the last segment close and be recognised.
    std::thread::sleep(Duration::from_secs(8));
    stop.store(true, Ordering::Relaxed);
    for b in busy {
        let _ = b.join();
    }

    let c = collector.0.lock().unwrap();
    let stats = c.stats.clone().unwrap_or_else(|| speech.stats());
    let rejected = |r: RejectReason| stats.rejected.get(&r).copied().unwrap_or(0);
    let col = |f: fn(&Transcript) -> u32| -> (u32, u32) {
        let mut v: Vec<u32> = c.finals.iter().map(f).collect();
        pct(&mut v)
    };
    let total = col(|t| t.latency_ms);
    let close = col(|t| t.close_ms);
    let queue = col(|t| t.queue_ms);
    let asr = col(|t| t.asr_ms);
    println!("| Étape | p50 ms | p95 ms |\n|---|---|---|");
    println!(
        "| Fin de voix → segment clos (silence + pas VAD) | {} | {} |",
        close.0, close.1
    );
    println!(
        "| Attente avant la reconnaissance (file, ligne provisoire en cours) | {} | {} |",
        queue.0, queue.1
    );
    println!("| Reconnaissance | {} | {} |", asr.0, asr.1);
    println!(
        "| **Total : fin de voix → texte prêt** | **{}** | **{}** |",
        total.0, total.1
    );
    println!(
        "\nLignes finales {} sur {} clips · lignes provisoires {} · abandonnées : trop tard {}, file pleine {} · segments {}",
        c.finals.len(),
        clips.len(),
        c.partials,
        rejected(RejectReason::Stale),
        rejected(RejectReason::Overflow),
        stats.segments
    );
}
