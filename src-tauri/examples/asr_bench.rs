//! `crosstalk-asr-bench`: offline and live evaluation of the speech pipeline
//! (the app's exact chain: 16 kHz conditioning, Silero VAD, segmenter, Whisper,
//! filter).
//!
//! ```text
//! # Reference corpus: TSV lines "path<TAB>lang<TAB>reference text"
//! crosstalk-asr-bench --manifest corpus/manifest.tsv --model base,small [--scenario clean,combat]
//! # Your own CS2 recordings (already noisy; reference optional)
//! crosstalk-asr-bench --manifest mes-enregistrements.tsv --scenario clean
//! # False transcripts on synthetic shots/explosions only (no voice)
//! crosstalk-asr-bench --noise-only 120 --model base
//! # Live: real CS2 capture, transcripts printed, latency and CPU measured (Windows)
//! crosstalk-asr-bench --live 120 --model base
//! ```
//!
//! `--full-encoder` encodes Whisper's full 30 s window instead of the segment
//! duration (the app's default), to compare speed and accuracy.
//! `--vad-only` runs the VAD and segmenter without Whisper (no model needed).
//! Options: `--models-dir DIR` (default: the app's folder), `--threads N`,
//! `--lang ru,en`, `--vad-threshold 0.55`, `--combat-peak-db -3`, `--markdown out.md`.
//! `--verbose` prints raw offline results, rejection reasons, language/full timings.
//! Live diagnostics: set CROSSTALK_SPEECH_DIAGNOSTICS=1 (stderr; raw text included).
//! `--live` requires CS2 process capture; there is no output-device fallback.
//! Nothing is uploaded; audio is never written unless `--export-mix DIR` is given.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crosstalk_lib::speech::bench::{
    char_errors, combat_track, parse_langs, peak_db, read_wav, run_pipeline, word_errors,
    write_wav_i16, SegmentResult,
};
use crosstalk_lib::speech::cpu::process_cpu_time;
use crosstalk_lib::speech::engine::{default_threads, logical_threads, Recognizer};
use crosstalk_lib::speech::vad::VoiceDetector;
use crosstalk_lib::speech::{
    models, EngineLoader, Lang, RejectReason, SpeechConfig, WhisperLoader,
};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

struct Clip {
    path: PathBuf,
    lang: Option<Lang>,
    reference: String,
}

fn read_manifest(path: &Path) -> Vec<Clip> {
    let base = path.parent().unwrap_or(Path::new("."));
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut cols = l.split('\t');
            let p = cols.next().unwrap_or_default();
            Clip {
                path: base.join(p),
                lang: cols.next().and_then(Lang::from_code),
                reference: cols.next().unwrap_or_default().to_string(),
            }
        })
        .collect()
}

fn fail(msg: &str) -> ! {
    eprintln!("erreur : {msg}");
    std::process::exit(2)
}

#[derive(Default)]
struct Totals {
    word_err: [usize; 2],
    words: [usize; 2],
    char_err: [usize; 2],
    chars: [usize; 2],
    audio_s: f64,
    asr_ms: Vec<u32>,
    latency_ms: Vec<u32>,
    rtf: Vec<f32>,
    accepted: usize,
    rejected: std::collections::BTreeMap<RejectReason, usize>,
    lang_ok: usize,
    lang_total: usize,
    cpu_s: f64,
    wall_s: f64,
}

fn pct(v: &[u32], q: f32) -> u32 {
    if v.is_empty() {
        return 0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    s[((q * s.len() as f32) as usize).min(s.len() - 1)]
}

fn lang_idx(l: Lang) -> usize {
    match l {
        Lang::Ru => 0,
        Lang::En => 1,
    }
}

/// Post-roll (13 frames) + worst VAD hop (192 ms) before Whisper starts.
const PIPELINE_DELAY_MS: u32 = 13 * 32 + 192;

fn account(t: &mut Totals, results: &[SegmentResult], clip_lang: Option<Lang>) -> String {
    let mut hyp = Vec::new();
    for r in results {
        match &r.outcome {
            Ok(text) => {
                t.accepted += 1;
                hyp.push(text.clone());
                if let (Some(want), Some(rec)) = (clip_lang, &r.recognition) {
                    t.lang_total += 1;
                    t.lang_ok += usize::from(rec.lang == want);
                }
            }
            Err(reason) => *t.rejected.entry(*reason).or_default() += 1,
        }
        if r.recognition.is_some() {
            t.asr_ms.push(r.asr_ms);
            t.latency_ms.push(PIPELINE_DELAY_MS + r.asr_ms);
            t.rtf.push(r.asr_ms as f32 / r.audio_ms.max(1) as f32);
        }
    }
    hyp.join(" ")
}

#[allow(clippy::too_many_arguments)]
fn run_corpus(
    clips: &[Clip],
    scenario: &str,
    peak: f32,
    detector: &mut dyn VoiceDetector,
    recognizer: &mut dyn Recognizer,
    config: &SpeechConfig,
    threads: usize,
    export: Option<&Path>,
    verbose: bool,
) -> Totals {
    let mut t = Totals::default();
    let (cpu0, wall0) = (process_cpu_time(), Instant::now());
    for (i, clip) in clips.iter().enumerate() {
        let bytes = std::fs::read(&clip.path)
            .unwrap_or_else(|e| fail(&format!("{}: {e}", clip.path.display())));
        let wav =
            read_wav(&bytes).unwrap_or_else(|e| fail(&format!("{}: {e}", clip.path.display())));
        let mut mono = wav.mono;
        if scenario == "combat" {
            // Voice peak normalised to -6 dBFS, game sounds on top at `peak`.
            let gain = 10f32.powf((-6.0 - peak_db(&mono)) / 20.0);
            mono.iter_mut().for_each(|s| *s *= gain);
            let noise = combat_track(mono.len(), wav.rate, peak, 1000 + i as u64);
            crosstalk_lib::speech::bench::mix(&mut mono, &noise);
        }
        if let Some(dir) = export {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(
                dir.join(format!("{scenario}-{i:03}.wav")),
                write_wav_i16(&mono, wav.rate),
            );
        }
        t.audio_s += mono.len() as f64 / wav.rate as f64;
        let results = run_pipeline(&mono, wav.rate, detector, Some(recognizer), config, threads)
            .unwrap_or_else(|e| fail(&e.to_string()));
        let hyp = account(&mut t, &results, clip.lang);
        if let (Some(l), false) = (clip.lang, clip.reference.is_empty()) {
            let (we, w) = word_errors(&clip.reference, &hyp);
            let (ce, c) = char_errors(&clip.reference, &hyp);
            let k = lang_idx(l);
            t.word_err[k] += we;
            t.words[k] += w;
            t.char_err[k] += ce;
            t.chars[k] += c;
        }
        if verbose {
            println!(
                "  [{scenario}] {} → « {hyp} »",
                clip.path.file_name().unwrap_or_default().to_string_lossy()
            );
            for r in &results {
                println!(
                    "      audio_ms={} inference_ms={} timing={:?} outcome={:?} recognition={:?}",
                    r.audio_ms, r.asr_ms, r.timing, r.outcome, r.recognition
                );
            }
        }
    }
    t.wall_s = wall0.elapsed().as_secs_f64();
    t.cpu_s = match (cpu0, process_cpu_time()) {
        (Some(a), Some(b)) => (b - a).as_secs_f64(),
        _ => f64::NAN,
    };
    t
}

fn rate(err: usize, n: usize) -> String {
    if n == 0 {
        "—".into()
    } else {
        format!("{:.1} %", 100.0 * err as f64 / n as f64)
    }
}

fn rejected(t: &Totals) -> String {
    if t.rejected.is_empty() {
        return "aucun".into();
    }
    t.rejected
        .iter()
        .map(|(r, n)| format!("{r:?} {n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn row(model: &str, scenario: &str, t: &Totals) -> String {
    let mean_rtf = if t.rtf.is_empty() {
        0.0
    } else {
        t.rtf.iter().sum::<f32>() / t.rtf.len() as f32
    };
    format!(
        "| {model} | {scenario} | {} | {} | {} | {} | {} | {} / {} ms | {} / {} ms | {:.2} | {:.0} % | {} |",
        rate(t.word_err[0], t.words[0]),
        rate(t.char_err[0], t.chars[0]),
        rate(t.word_err[1], t.words[1]),
        rate(t.char_err[1], t.chars[1]),
        if t.lang_total == 0 { "—".into() } else { format!("{}/{}", t.lang_ok, t.lang_total) },
        pct(&t.asr_ms, 0.5),
        pct(&t.asr_ms, 0.95),
        pct(&t.latency_ms, 0.5),
        pct(&t.latency_ms, 0.95),
        mean_rtf,
        // CPU cores busy while processing, relative to real time.
        100.0 * t.cpu_s / t.audio_s.max(1e-9),
        rejected(t),
    )
}

const HEADER: &str = "| Modèle | Scénario | WER ru | CER ru | WER en | CER en | Langue ok | Whisper p50 / p95 | Latence estimée p50 / p95 | RTF | CPU / temps réel | Segments rejetés |\n|---|---|---|---|---|---|---|---|---|---|---|---|";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "{}",
            include_str!("asr_bench.rs")
                .lines()
                .take_while(|line| line.starts_with("//!"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        return;
    }
    let dir = arg(&args, "--models-dir")
        .map(PathBuf::from)
        .unwrap_or_else(models::models_dir);
    let threads = arg(&args, "--threads")
        .map(|t| {
            let n = t
                .parse::<usize>()
                .unwrap_or_else(|_| fail("--threads exige un entier positif"));
            if n == 0 || n > logical_threads() {
                fail(&format!(
                    "--threads doit être entre 1 et {}",
                    logical_threads()
                ));
            }
            n
        })
        .unwrap_or_else(default_threads);
    let mut config = SpeechConfig::default();
    if let Some(l) = arg(&args, "--lang") {
        config.languages = parse_langs(&l);
    }
    config.fast_encoder = !args.iter().any(|a| a == "--full-encoder");
    if let Some(v) = arg(&args, "--vad-threshold").and_then(|v| v.parse().ok()) {
        config.vad_threshold = v;
    }
    let config = config.sanitized();
    let peak_db: f32 = arg(&args, "--combat-peak-db")
        .and_then(|v| v.parse().ok())
        .unwrap_or(-3.0);
    let peak = 10f32.powf(peak_db / 20.0);
    let model_list = arg(&args, "--model").unwrap_or_else(|| models::DEFAULT_MODEL.into());
    let verbose = args.iter().any(|a| a == "--verbose");

    println!(
        "Crosstalk ASR bench · modèles : {} · threads : {threads} · langues : {:?}",
        dir.display(),
        config.languages
    );
    let loader = WhisperLoader;
    let mut detector = loader
        .load_detector(&dir)
        .unwrap_or_else(|e| fail(&format!("VAD : {e}")));

    if let Some(secs) = arg(&args, "--live").and_then(|s| s.parse::<u64>().ok()) {
        return live(secs, &dir, &model_list, config, threads);
    }

    if args.iter().any(|a| a == "--vad-only") {
        return vad_only(&args, detector.as_mut(), &config, peak);
    }

    let mut md = String::new();
    md.push_str(&format!(
        "### Banc d'essai ASR\n\nMachine : {} threads logiques, Whisper sur {threads} threads, CPU (AVX2), sans GPU, encodeur {}. Bruit de combat **synthétique** (crête {peak_db} dBFS, voix à −6 dBFS).\n\n",
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        if config.fast_encoder { "réduit à la durée du segment" } else { "fenêtre complète de 30 s" },
    ));

    for model in model_list
        .split(',')
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        let t0 = Instant::now();
        let mut recognizer = match loader.load_recognizer(&dir, model) {
            Ok(r) => r,
            Err(e) => fail(&format!("modèle {model} : {e}")),
        };
        let load_ms = t0.elapsed().as_millis();
        println!("\n== whisper {model} (chargé en {load_ms} ms)");
        let config = SpeechConfig {
            model: model.into(),
            ..config.clone()
        };

        if let Some(secs) = arg(&args, "--noise-only").and_then(|s| s.parse::<u64>().ok()) {
            let rate = 48_000;
            let noise = combat_track((secs * rate as u64) as usize, rate, peak, 42);
            let res = run_pipeline(
                &noise,
                rate,
                detector.as_mut(),
                Some(recognizer.as_mut()),
                &config,
                threads,
            )
            .unwrap_or_else(|e| fail(&e.to_string()));
            let mut t = Totals {
                audio_s: secs as f64,
                ..Default::default()
            };
            account(&mut t, &res, None);
            let to_whisper = res.iter().filter(|r| r.recognition.is_some()).count();
            let line = format!(
                "Bruit seul ({secs} s, tirs + explosions synthétiques) : {} segments détectés, {to_whisper} envoyés à Whisper, **{} fausses transcriptions affichées** — rejets : {}",
                res.len(),
                t.accepted,
                rejected(&t)
            );
            println!("{line}");
            for r in res.iter().filter(|r| r.outcome.is_ok()) {
                println!(
                    "  fausse transcription : « {} »",
                    r.outcome.as_ref().unwrap()
                );
            }
            md.push_str(&format!("- `{model}` — {line}\n"));
        }

        if let Some(manifest) = arg(&args, "--manifest") {
            let clips = read_manifest(Path::new(&manifest));
            let scenarios = arg(&args, "--scenario").unwrap_or_else(|| "clean,combat".into());
            if !md.contains("| Modèle |") {
                md.push_str(&format!(
                    "\nCorpus : {} extraits ({manifest}).\n\n{HEADER}\n",
                    clips.len()
                ));
            }
            for scenario in scenarios.split(',').map(str::trim) {
                let export = arg(&args, "--export-mix").map(PathBuf::from);
                let t = run_corpus(
                    &clips,
                    scenario,
                    peak,
                    detector.as_mut(),
                    recognizer.as_mut(),
                    &config,
                    threads,
                    export.as_deref(),
                    verbose,
                );
                let label = if config.fast_encoder {
                    model.to_string()
                } else {
                    format!("{model} (fenêtre 30 s)")
                };
                let r = row(&label, scenario, &t);
                println!(
                    "{HEADER}\n{r}\n  ({:.0} s d'audio traités en {:.0} s)",
                    t.audio_s, t.wall_s
                );
                md.push_str(&r);
                md.push('\n');
                // Written after every row: a time limit keeps what was measured.
                if let Some(out) = arg(&args, "--markdown") {
                    let _ = std::fs::write(&out, &md);
                }
            }
        }
    }
    md.push_str("\nLatence estimée = post-roll 416 ms + pas de VAD 192 ms + calcul Whisper (hors file d'attente). CPU / temps réel : temps CPU du processus ÷ durée audio (100 % = un cœur occupé en continu).\n");
    if let Some(out) = arg(&args, "--markdown") {
        std::fs::write(&out, &md).unwrap_or_else(|e| fail(&format!("{out}: {e}")));
        println!("\nRapport écrit dans {out}");
    }
}

/// VAD + segmenter only (no Whisper model needed): what reaches Whisper.
fn vad_only(args: &[String], detector: &mut dyn VoiceDetector, config: &SpeechConfig, peak: f32) {
    let summarize = |name: &str, res: &[SegmentResult], audio_s: f64| {
        let sent: Vec<_> = res.iter().filter(|r| r.outcome.is_ok()).collect();
        let speech_ms: u32 = sent.iter().map(|r| r.audio_ms).sum();
        let mut rej = std::collections::BTreeMap::new();
        for r in res {
            if let Err(e) = r.outcome {
                *rej.entry(e).or_insert(0) += 1;
            }
        }
        let mut out = format!(
            "{name} : {:.1} s d'audio → {} segments envoyés à Whisper ({:.1} s), rejetés avant Whisper : {:?}\n",
            audio_s,
            sent.len(),
            speech_ms as f64 / 1000.0,
            rej
        );
        for r in &sent {
            out.push_str(&format!(
                "    segment à {:.2} s, durée {:.2} s\n",
                r.start_ms as f64 / 1000.0,
                r.audio_ms as f64 / 1000.0
            ));
        }
        print!("{out}");
        out
    };
    let mut report = String::from("### VAD seule (sans Whisper)\n\n```\n");
    if let Some(secs) = arg(args, "--noise-only").and_then(|s| s.parse::<u64>().ok()) {
        let rate = 48_000;
        let noise = combat_track((secs * rate as u64) as usize, rate, peak, 42);
        let res = run_pipeline(&noise, rate, detector, None, config, 1)
            .unwrap_or_else(|e| fail(&e.to_string()));
        report.push_str(&summarize("Bruit seul (synthétique)", &res, secs as f64));
    }
    if let Some(manifest) = arg(args, "--manifest") {
        let scenarios = arg(args, "--scenario").unwrap_or_else(|| "clean,combat".into());
        for clip in read_manifest(Path::new(&manifest)) {
            let wav = read_wav(&std::fs::read(&clip.path).unwrap_or_else(|e| fail(&e.to_string())))
                .unwrap_or_else(|e| fail(&e));
            for scenario in scenarios.split(',').map(str::trim) {
                let mut mono = wav.mono.clone();
                if scenario == "combat" {
                    let gain = 10f32.powf((-6.0 - peak_db(&mono)) / 20.0);
                    mono.iter_mut().for_each(|s| *s *= gain);
                    let noise = combat_track(mono.len(), wav.rate, peak, 1000);
                    crosstalk_lib::speech::bench::mix(&mut mono, &noise);
                }
                let res = run_pipeline(&mono, wav.rate, detector, None, config, 1)
                    .unwrap_or_else(|e| fail(&e.to_string()));
                let name = format!("{} [{scenario}]", clip.path.display());
                report.push_str(&summarize(&name, &res, mono.len() as f64 / wav.rate as f64));
            }
        }
    }
    report.push_str("```\n");
    if let Some(out) = arg(args, "--markdown") {
        std::fs::write(&out, &report).unwrap_or_else(|e| fail(&format!("{out}: {e}")));
        println!("\nRapport écrit dans {out}");
    }
}

/// Real capture of CS2 (or the output device) through the app's services.
fn live(secs: u64, dir: &Path, model: &str, config: SpeechConfig, threads: usize) {
    use crosstalk_lib::audio::{self, AudioService, CaptureMode, StopReason};
    use crosstalk_lib::game::Cs2Watcher;
    use crosstalk_lib::speech::{
        SpeechEvents, SpeechService, SpeechStats, SpeechStatus, Transcript, VoiceActivity,
    };

    struct Console(Mutex<Option<SpeechStats>>);
    impl SpeechEvents for Console {
        fn status(&self, s: &SpeechStatus) {
            println!(
                "[moteur] {:?}{}",
                s.state,
                s.error
                    .as_ref()
                    .map(|e| format!(" — {e}"))
                    .unwrap_or_default()
            );
        }
        fn transcript(&self, t: &Transcript) {
            println!(
                "[{}] « {} »  (audio {} ms · Whisper {} ms · latence {} ms · logprob {:.2})",
                t.lang.code().to_uppercase(),
                t.text,
                t.audio_ms,
                t.asr_ms,
                t.latency_ms,
                t.avg_logprob
            );
        }
        fn activity(&self, a: VoiceActivity) {
            if a.speaking {
                print!("…");
                let _ = std::io::stdout().flush();
            }
        }
        fn stats(&self, s: &SpeechStats) {
            *self.0.lock().unwrap() = Some(s.clone());
        }
    }
    struct Quiet;
    impl audio::AudioEvents for Quiet {
        fn status(&self, s: &audio::CaptureStatus) {
            println!("[capture] {:?} {:?}", s.state, s.source);
        }
        fn level(&self, _r: audio::LevelReading) {}
    }

    let console = Arc::new(Console(Mutex::new(None)));
    let speech = SpeechService::start_with_threads(
        console.clone(),
        Arc::new(WhisperLoader),
        dir.to_path_buf(),
        SpeechConfig {
            model: model.split(',').next().unwrap_or("base").into(),
            ..config
        },
        threads,
    );
    let end = Instant::now() + Duration::from_secs(60);
    while speech.status().state == crosstalk_lib::speech::EngineState::Loading
        && Instant::now() < end
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    if speech.status().state != crosstalk_lib::speech::EngineState::Ready {
        fail(&format!("moteur indisponible : {:?}", speech.status()));
    }
    let watcher = Cs2Watcher::default();
    watcher.poll();
    let audio = AudioService::new(audio::default_backend(), Arc::new(Quiet));
    audio.set_sink(Some(Box::new(speech.sink())));
    let pid = watcher
        .status()
        .pid
        .unwrap_or_else(|| fail("CS2 absent : --live exige cs2.exe"));
    let st = audio.start(CaptureMode::Process, None, Some(pid));
    if st.state != audio::CaptureState::Capturing {
        fail(&format!("capture impossible : {:?}", st.error));
    }
    println!("Écoute pendant {secs} s… (parlez ou jouez ; rien n'est enregistré)");
    let (cpu0, wall0) = (process_cpu_time(), Instant::now());
    std::thread::sleep(Duration::from_secs(secs));
    let cpu = match (cpu0, process_cpu_time()) {
        (Some(a), Some(b)) => (b - a).as_secs_f64() / wall0.elapsed().as_secs_f64() * 100.0,
        _ => f64::NAN,
    };
    audio.stop(StopReason::User);
    speech.purge();
    println!("Arrêt : les segments en attente et résultats encore en calcul sont purgés ; bilan des résultats terminés avant arrêt.");
    let s = speech.stats();
    println!(
        "\nRésultat : {} segments, {} transcrits, rejets {:?}, latence p50 {:?} ms / p95 {:?} ms, blocs perdus {}, CPU moyen {:.0} % d'un cœur ({:.1} % de la machine)",
        s.segments,
        s.transcribed,
        s.rejected,
        s.latency_p50_ms,
        s.latency_p95_ms,
        s.dropped_blocks,
        cpu,
        cpu / std::thread::available_parallelism().map(|n| n.get() as f64).unwrap_or(1.0)
    );
}
