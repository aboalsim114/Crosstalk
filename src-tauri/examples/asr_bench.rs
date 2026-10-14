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
//! `--gpu-info` prints the Vulkan GPU Whisper would use (or none) and exits.
//! `--accel cpu|gpu|auto` (default cpu): where Whisper runs (Vulkan builds;
//! `auto` measures both and keeps the GPU if clearly faster).
//! `--full-encoder` encodes Whisper's full 30 s window instead of the segment
//! duration (the app's default), to compare speed and accuracy.
//! `--vad-only` runs the VAD and segmenter without Whisper (no model needed).
//!
//! crosstalk-asr-bench --manifest corpus/manifest.tsv --compare-fronts plain,focus,focus+band --model base [--repeat 4]
//!
//! compares audio front-ends on the same synthetic stereo CS2 scenes (voice in
//! the centre, spatialised combat around it, combat-only gaps before and
//! after): word/char error rates, voice lines missed, false subtitles in the
//! gaps, rejections, Whisper time, front-end cost. With `--vad-only`: what
//! reaches Whisper, no model needed. Fronts: plain (A, current), focus (B,
//! "Focus voix"), band (200 Hz voice band), focus+band, and for evaluation
//! only rnnoise / focus+rnnoise (C: RNNoise denoiser, bench dependency).
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
    apply_front, char_errors, combat_scene, combat_track, parse_langs, peak_db, read_wav,
    run_pipeline, score_scene, voice_snr_db, word_errors, write_wav_i16, Front, SegmentResult,
    SCENE_RATE,
};
use crosstalk_lib::speech::cpu::process_cpu_time;
use crosstalk_lib::speech::engine::{default_threads, logical_threads, Recognizer};
use crosstalk_lib::speech::vad::VoiceDetector;
use crosstalk_lib::speech::{
    models, Accel, EngineLoader, Lang, RejectReason, SpeechConfig, WhisperLoader,
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
    #[cfg(feature = "whisper")]
    if args.iter().any(|a| a == "--gpu-info") {
        match crosstalk_lib::speech::engine::vulkan_gpu() {
            Some(name) => println!("GPU Vulkan : {name}"),
            None => println!("GPU Vulkan : aucun (Whisper sur le processeur)"),
        }
        return;
    }
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
    config.accel = match arg(&args, "--accel").as_deref() {
        Some("gpu") => Accel::Gpu,
        Some("auto") => Accel::Auto,
        _ => Accel::Cpu,
    };
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

    if let Some(list) = arg(&args, "--compare-fronts") {
        return compare_fronts(
            &args,
            &list,
            &dir,
            &model_list,
            detector.as_mut(),
            &config,
            threads,
            peak,
        );
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
        let mut recognizer = match loader.load_recognizer(&dir, model, config.accel) {
            Ok(r) => r,
            Err(e) => fail(&format!("modèle {model} : {e}")),
        };
        let load_ms = t0.elapsed().as_millis();
        let backend = recognizer.backend().unwrap_or_else(|| "CPU".into());
        println!("\n== whisper {model} · {backend} (chargé en {load_ms} ms)");
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
            if t.partial {
                println!("  … [{}] {}", t.lang.code().to_uppercase(), t.text);
                return;
            }
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
        fn retract(&self, _id: u64) {
            println!("  (transcription partielle retirée)");
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

/// A front-end of the comparison: the library's, or a denoiser evaluated
/// here only (not in the app).
#[derive(Debug, Clone, Copy)]
enum Fe {
    Lib(Front),
    /// C: RNNoise (nnnoiseless, BSD-3, weights in the crate), after the
    /// focus or alone.
    Denoise {
        focus: bool,
    },
}

impl Fe {
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "rnnoise" | "c" => Some(Fe::Denoise { focus: false }),
            "focus+rnnoise" => Some(Fe::Denoise { focus: true }),
            other => Front::parse(other).map(Fe::Lib),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Fe::Lib(f) => f.label(),
            Fe::Denoise { focus: false } => "C · RNNoise",
            Fe::Denoise { focus: true } => "B + RNNoise",
        }
    }
    /// The library front whose linear part applies to the SNR reference.
    fn reference(self) -> Front {
        match self {
            Fe::Lib(f) => f,
            Fe::Denoise { .. } => Front::Plain,
        }
    }
    fn apply(self, stereo: &[f32]) -> Vec<f32> {
        match self {
            Fe::Lib(f) => apply_front(stereo, SCENE_RATE, f),
            Fe::Denoise { focus } => {
                let base = apply_front(
                    stereo,
                    SCENE_RATE,
                    if focus { Front::Focus } else { Front::Plain },
                );
                rnnoise(&base)
            }
        }
    }
}

/// RNNoise at 48 kHz (10 ms frames, i16 scale), delay compensated.
fn rnnoise(x: &[f32]) -> Vec<f32> {
    use nnnoiseless::DenoiseState;
    let n = DenoiseState::FRAME_SIZE;
    let mut st = DenoiseState::new();
    let mut out = Vec::with_capacity(x.len() + n);
    let (mut inp, mut buf) = (vec![0.0f32; n], vec![0.0f32; n]);
    for chunk in x.chunks(n) {
        inp.fill(0.0);
        for (d, s) in inp.iter_mut().zip(chunk) {
            *d = s * 32768.0;
        }
        st.process_frame(&mut buf, &inp);
        out.extend(buf.iter().map(|s| s / 32768.0));
    }
    let mut out = out.split_off(n.min(out.len()));
    out.resize(x.len(), 0.0);
    out
}

#[derive(Default)]
struct FrontTotals {
    t: Totals,
    scenes: usize,
    missed: usize,
    false_alarms: usize,
    front_s: f64,
    coverage: f64,
    snr_db: f64,
}

/// A/B(/C) comparison of audio front-ends on identical stereo combat scenes.
#[allow(clippy::too_many_arguments)]
fn compare_fronts(
    args: &[String],
    list: &str,
    dir: &Path,
    model_list: &str,
    detector: &mut dyn VoiceDetector,
    config: &SpeechConfig,
    threads: usize,
    peak: f32,
) {
    let fronts: Vec<Fe> = list
        .split(',')
        .map(|f| Fe::parse(f).unwrap_or_else(|| fail(&format!("traitement inconnu : {f}"))))
        .collect();
    let manifest =
        arg(args, "--manifest").unwrap_or_else(|| fail("--compare-fronts exige --manifest"));
    let clips = read_manifest(Path::new(&manifest));
    let repeat: u64 = arg(args, "--repeat")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let vad_only = args.iter().any(|a| a == "--vad-only");
    let models: Vec<Option<&str>> = if vad_only {
        vec![None]
    } else {
        model_list
            .split(',')
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(Some)
            .collect()
    };
    let mut md = format!(
        "### Comparaison des traitements audio\n\nScènes **synthétiques** : {} extrait(s) × {repeat} tirage(s), voix au centre (−6 dBFS), combat spatialisé (crête {:.1} dBFS) avant, pendant et après chaque phrase. {}.\n\n| Modèle | Scène | Traitement | WER ru | WER en | CER ru | CER en | Voix manquées | Couverture | Faux positifs | RSB voix | Rejets | Whisper p50 / p95 | Traitement audio | CPU / temps réel |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
        clips.len(),
        20.0 * peak.log10(),
        if vad_only { "Sans Whisper : « voix manquées » = aucun segment envoyé à Whisper sur la phrase ; « faux positifs » = segments envoyés pendant le combat seul" } else { "« Faux positifs » = sous-titres acceptés pendant le combat seul" }
    );
    for model in models {
        let mut recognizer = model.map(|m| {
            WhisperLoader
                .load_recognizer(dir, m, config.accel)
                .unwrap_or_else(|e| fail(&format!("modèle {m} : {e}")))
        });
        let config = SpeechConfig {
            model: model.unwrap_or("—").into(),
            ..config.clone()
        };
        for (scene_name, own) in [("combat autour", false), ("+ ses propres tirs", true)] {
            for &front in &fronts {
                let mut ft = FrontTotals::default();
                let (cpu0, wall0) = (process_cpu_time(), Instant::now());
                for (i, clip) in clips.iter().enumerate() {
                    let bytes = std::fs::read(&clip.path)
                        .unwrap_or_else(|e| fail(&format!("{}: {e}", clip.path.display())));
                    let wav = read_wav(&bytes)
                        .unwrap_or_else(|e| fail(&format!("{}: {e}", clip.path.display())));
                    for r in 0..repeat {
                        // Same seed for every front: the same recordings.
                        let sc = combat_scene(
                            &wav.mono,
                            wav.rate,
                            peak,
                            7000 + 31 * i as u64 + 1009 * r,
                            own,
                        );
                        let tf = Instant::now();
                        let mono = front.apply(&sc.stereo);
                        ft.front_s += tf.elapsed().as_secs_f64();
                        ft.snr_db += voice_snr_db(
                            &sc.clean,
                            &mono,
                            sc.voice_ms,
                            SCENE_RATE,
                            front.reference(),
                        ) as f64;
                        ft.t.audio_s += mono.len() as f64 / SCENE_RATE as f64;
                        let results = run_pipeline(
                            &mono,
                            SCENE_RATE,
                            detector,
                            recognizer
                                .as_mut()
                                .map(|r| r.as_mut() as &mut dyn Recognizer),
                            &config,
                            threads,
                        )
                        .unwrap_or_else(|e| fail(&e.to_string()));
                        account(&mut ft.t, &results, clip.lang);
                        let s = score_scene(&results, sc.voice_ms, recognizer.is_some());
                        ft.scenes += 1;
                        ft.missed += usize::from(s.missed);
                        ft.false_alarms += s.false_alarms;
                        ft.coverage += s.coverage as f64;
                        if let (Some(l), true) = (
                            clip.lang,
                            recognizer.is_some() && !clip.reference.is_empty(),
                        ) {
                            let (we, w) = word_errors(&clip.reference, &s.text);
                            let (ce, c) = char_errors(&clip.reference, &s.text);
                            let k = lang_idx(l);
                            ft.t.word_err[k] += we;
                            ft.t.words[k] += w;
                            ft.t.char_err[k] += ce;
                            ft.t.chars[k] += c;
                        }
                        if args.iter().any(|a| a == "--verbose") {
                            println!(
                                "  [{scene_name} · {}] « {} » (manquée : {}, faux positifs : {})",
                                front.label(),
                                s.text,
                                s.missed,
                                s.false_alarms
                            );
                        }
                    }
                }
                ft.t.wall_s = wall0.elapsed().as_secs_f64();
                ft.t.cpu_s = match (cpu0, process_cpu_time()) {
                    (Some(a), Some(b)) => (b - a).as_secs_f64(),
                    _ => f64::NAN,
                };
                let t = &ft.t;
                let line = format!(
                    "| {} | {scene_name} | {} | {} | {} | {} | {} | {}/{} | {:.0} % | {} | {:.1} dB | {} | {} | {:.2} ms/s | {:.0} % |\n",
                    model.unwrap_or("VAD seule"),
                    front.label(),
                    rate(t.word_err[0], t.words[0]),
                    rate(t.word_err[1], t.words[1]),
                    rate(t.char_err[0], t.chars[0]),
                    rate(t.char_err[1], t.chars[1]),
                    ft.missed,
                    ft.scenes,
                    100.0 * ft.coverage / ft.scenes.max(1) as f64,
                    ft.false_alarms,
                    ft.snr_db / ft.scenes.max(1) as f64,
                    rejected(t),
                    if t.asr_ms.is_empty() { "—".into() } else { format!("{} / {} ms", pct(&t.asr_ms, 0.5), pct(&t.asr_ms, 0.95)) },
                    1000.0 * ft.front_s / t.audio_s.max(1e-9),
                    100.0 * t.cpu_s / t.audio_s.max(1e-9),
                );
                print!("{line}");
                md.push_str(&line);
                if let Some(out) = arg(args, "--markdown") {
                    let _ = std::fs::write(&out, &md);
                }
            }
        }
    }
    md.push_str("\nTraitement audio : temps de calcul du traitement par seconde d'audio (48 kHz stéréo). CPU / temps réel : tout le pipeline (traitement, VAD, Whisper).\n");
    if let Some(out) = arg(args, "--markdown") {
        std::fs::write(&out, &md).unwrap_or_else(|e| fail(&format!("{out}: {e}")));
        println!("\nRapport écrit dans {out}");
    }
}
