//! crosstalk-translate-bench: Opus-MT against TranslateGemma on a Counter-
//! Strike corpus (tests/data/translation-corpus.tsv, invented lines).
//!
//!   crosstalk-translate-bench [--corpus translation-corpus.tsv]
//!       [--engines opus,translategemma] [--backend auto|cpu|gpu] [--threads N]
//!       [--models-dir DIR] [--llm-exe crosstalk-llm.exe] [--repeat 3]
//!       [--transcripts whisper.tsv] [--markdown rapport.md]
//!
//! Raw models, as the app calls them for lines its lexicon does not know
//! (the lexicon column says which lines never reach a model in the app).
//! Measured: load time, latency p50 / p95, memory, CPU, video memory
//! (TranslateGemma on the GPU), kept terms (numbers, A/B, maps, weapons),
//! suspicious outputs (empty, untranslated, commentary, much too long).
//! Meaning is judged by reading the table against the reference.
//!
//! `--transcripts`: TSV "id<TAB>Whisper text" for the same ids (Whisper's
//! output on recordings of the corpus): each line is translated from the
//! exact text and from Whisper's, so an error is attributed to the
//! transcription or to the translation. Nothing is sent online.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crosstalk_lib::translation::callouts;
use crosstalk_lib::translation::llm::{catalog, process, prompt};
use crosstalk_lib::translation::marian::{Marian, Mode};
use crosstalk_lib::translation::models;

struct Line {
    id: String,
    lang: String,
    cat: String,
    text: String,
    reference: String,
    keep: Vec<Vec<String>>,
    whisper: Option<String>,
}

#[derive(Default, Clone)]
struct Out {
    text: String,
    ms: Vec<u32>,
    error: Option<String>,
}

struct EngineRun {
    name: &'static str,
    load_ms: u32,
    outs: Vec<Out>,
    whisper_outs: Vec<Option<Out>>,
    rss_mb: Option<u32>,
    cpu_pct: Option<f64>,
    vram_mb: Option<u64>,
    backend: String,
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn read_corpus(path: &Path) -> Vec<Line> {
    let raw =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("corpus {} : {e}", path.display()));
    raw.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("id\t") && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Line {
                id: f[0].into(),
                lang: f[1].into(),
                cat: f[2].into(),
                text: f[3].into(),
                reference: f.get(4).copied().unwrap_or_default().into(),
                keep: f
                    .get(5)
                    .copied()
                    .unwrap_or_default()
                    .split('|')
                    .filter(|g| !g.trim().is_empty())
                    .map(|g| g.split('/').map(|a| a.trim().to_lowercase()).collect())
                    .collect(),
                whisper: None,
            }
        })
        .collect()
}

/// Kept terms: every group has one of its accepted forms in the output.
fn kept(line: &Line, out: &str) -> (usize, usize) {
    let low = out.to_lowercase();
    let ok = line
        .keep
        .iter()
        .filter(|g| g.iter().any(|a| low.contains(a.as_str())))
        .count();
    (ok, line.keep.len())
}

/// Why an output looks wrong without reading it, if it does.
fn suspicious(line: &Line, out: &Out) -> Option<&'static str> {
    if out.error.is_some() {
        return Some("erreur");
    }
    let t = out.text.trim();
    if t.is_empty() {
        return Some("vide");
    }
    if t.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
        return Some("non traduit");
    }
    let low = t.to_lowercase();
    if t.contains('\n')
        || [
            "translation",
            "traduction",
            "voici",
            "note :",
            "note:",
            "remarque",
        ]
        .iter()
        .any(|w| low.contains(w))
    {
        return Some("commentaire");
    }
    let limit = 3 * line.reference.chars().count().max(12);
    if t.chars().count() > limit {
        return Some("trop long");
    }
    None
}

fn pct(sorted: &[u32], p: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

#[cfg(windows)]
fn own_rss_mb() -> u32 {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak: usize,
        working_set: usize,
        rest: [usize; 6],
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(p: isize, c: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters {
        cb: std::mem::size_of::<Counters>() as u32,
        ..Default::default()
    };
    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) } == 0 {
        return 0;
    }
    (c.working_set / (1024 * 1024)) as u32
}

#[cfg(not(windows))]
fn own_rss_mb() -> u32 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map_or(0, |p| (p * 4096 / (1024 * 1024)) as u32)
}

fn run_opus(lines: &[Line], dir: &Path, repeat: usize) -> Option<EngineRun> {
    let mut engines = std::collections::HashMap::new();
    let rss0 = own_rss_mb();
    let t = Instant::now();
    for lang in ["ru", "en"] {
        let pair = models::pair(lang)?;
        if models::missing_file(dir, pair).is_some() {
            eprintln!("Opus-MT {lang} → fr absent de {}", dir.display());
            return None;
        }
        engines.insert(
            lang,
            Marian::load(&models::pair_dir(dir, pair), Mode::Fast).expect("Opus-MT"),
        );
    }
    let load_ms = t.elapsed().as_millis() as u32;
    let cpu0 = crosstalk_lib::speech::cpu::process_cpu_time();
    let wall = Instant::now();
    let mut one = |lang: &str, text: &str| {
        let e = engines.get_mut(lang).expect("ru/en");
        let mut o = Out::default();
        for _ in 0..repeat {
            let t = Instant::now();
            match e.translate(text) {
                Ok(s) => o.text = callouts::clean_output(&s),
                Err(err) => o.error = Some(err),
            }
            o.ms.push(t.elapsed().as_millis() as u32);
        }
        o
    };
    let outs: Vec<Out> = lines.iter().map(|l| one(&l.lang, &l.text)).collect();
    let whisper_outs = lines
        .iter()
        .map(|l| l.whisper.as_ref().map(|w| one(&l.lang, w)))
        .collect();
    let cpu_pct = match (cpu0, crosstalk_lib::speech::cpu::process_cpu_time()) {
        (Some(a), Some(b)) => {
            Some((b - a).as_secs_f64() / wall.elapsed().as_secs_f64().max(1e-3) * 100.0)
        }
        _ => None,
    };
    Some(EngineRun {
        name: "Opus-MT",
        load_ms,
        outs,
        whisper_outs,
        rss_mb: Some(own_rss_mb().saturating_sub(rss0)),
        cpu_pct,
        vram_mb: None,
        backend: "processeur".into(),
    })
}

fn run_llm(
    lines: &[Line],
    dir: &Path,
    exe: PathBuf,
    backend: &str,
    threads: u32,
    repeat: usize,
) -> Option<EngineRun> {
    let m = catalog::default_model();
    let model = catalog::model_path(dir, m);
    if !model.is_file() {
        eprintln!("{} absent : {}", m.name, model.display());
        return None;
    }
    let launch = process::Launch {
        exe,
        model,
        threads: if threads > 0 {
            threads
        } else {
            (std::thread::available_parallelism().map_or(4, |n| n.get()) / 2).clamp(2, 6) as u32
        },
        gpu_layers: if backend == "cpu" { 0 } else { 99 },
        gpu_min_free_mb: if backend == "auto" {
            m.bytes / (1024 * 1024) + 1_536
        } else {
            0
        },
        ctx: m.ctx,
    };
    let t = Instant::now();
    let mut p = match process::LlmProcess::spawn(&launch, Duration::from_secs(300)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} : {e}", m.name);
            return None;
        }
    };
    let load_ms = t.elapsed().as_millis() as u32;
    let ready = p.ready.clone();
    let mut cpu = (None::<u64>, 0u64);
    let mut rss = ready.rss_mb;
    let wall = Instant::now();
    let mut one = |p: &mut process::LlmProcess, lang: &str, text: &str| {
        let mut o = Out::default();
        let Ok(pr) = prompt::translategemma(lang, "fr", text) else {
            o.error = Some("langue non gérée".into());
            return o;
        };
        for _ in 0..repeat {
            let t = Instant::now();
            match p.request(&pr, prompt::max_tokens(text), Duration::from_secs(60)) {
                Ok(r) => {
                    o.text = prompt::clean_output(text, r.text.as_deref().unwrap_or_default());
                    cpu.0.get_or_insert(r.cpu_ms);
                    cpu.1 = r.cpu_ms;
                    rss = rss.max(r.rss_mb);
                }
                Err(e) => o.error = Some(e),
            }
            o.ms.push(t.elapsed().as_millis() as u32);
        }
        o
    };
    let outs: Vec<Out> = lines
        .iter()
        .map(|l| one(&mut p, &l.lang, &l.text))
        .collect();
    let whisper_outs = lines
        .iter()
        .map(|l| l.whisper.as_ref().map(|w| one(&mut p, &l.lang, w)))
        .collect();
    let secs = wall.elapsed().as_secs_f64().max(1e-3);
    p.stop();
    Some(EngineRun {
        name: "TranslateGemma 4B",
        load_ms,
        outs,
        whisper_outs,
        rss_mb: Some(rss).filter(|&r| r > 0),
        cpu_pct: cpu.0.map(|a| (cpu.1 - a) as f64 / 1000.0 / secs * 100.0),
        vram_mb: ready.vram_used_mb,
        backend: if ready.backend == "cpu" {
            format!(
                "processeur, {} threads{}",
                ready.threads,
                ready
                    .gpu_skipped
                    .map(|s| format!(" (GPU non utilisé : {s})"))
                    .unwrap_or_default()
            )
        } else {
            format!("{} ({})", ready.device.unwrap_or_default(), ready.backend)
        },
    })
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let corpus = arg(&args, "--corpus")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let next = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("translation-corpus.tsv")));
            next.filter(|p| p.is_file()).unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/translation-corpus.tsv")
            })
        });
    let dir = arg(&args, "--models-dir")
        .map(PathBuf::from)
        .unwrap_or_else(crosstalk_lib::speech::models::models_dir);
    let engines = arg(&args, "--engines").unwrap_or_else(|| "opus,translategemma".into());
    let backend = arg(&args, "--backend").unwrap_or_else(|| "auto".into());
    let threads: u32 = arg(&args, "--threads")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let repeat: usize = arg(&args, "--repeat")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let exe = arg(&args, "--llm-exe")
        .map(PathBuf::from)
        .unwrap_or_else(process::helper_path);
    let mut lines = read_corpus(&corpus);
    if let Some(t) = arg(&args, "--transcripts") {
        let raw = std::fs::read_to_string(&t).unwrap_or_else(|e| panic!("{t} : {e}"));
        for l in raw.lines().filter(|l| !l.starts_with('#')) {
            if let Some((id, text)) = l.split_once('\t') {
                if let Some(line) = lines.iter_mut().find(|x| x.id == id) {
                    line.whisper = Some(text.trim().to_string());
                }
            }
        }
    }

    let mut runs = Vec::new();
    if engines.contains("opus") {
        runs.extend(run_opus(&lines, &dir, repeat));
    }
    if engines.contains("translategemma") {
        runs.extend(run_llm(&lines, &dir, exe, &backend, threads, repeat));
    }
    if runs.is_empty() {
        eprintln!("aucun moteur disponible (modèles dans {})", dir.display());
        std::process::exit(2);
    }

    let mut md = String::new();
    let lexicon = lines
        .iter()
        .filter(|l| callouts::translate(&l.lang, &l.text).is_some())
        .count();
    md += &format!(
        "# Banc de traduction Crosstalk\n\nCorpus : `{}` · {} lignes ({} ru, {} en) · {} passage(s) par ligne · {} lignes connues du lexique (jamais envoyées à un modèle dans l'application).\n\n",
        corpus.display(),
        lines.len(),
        lines.iter().filter(|l| l.lang == "ru").count(),
        lines.iter().filter(|l| l.lang == "en").count(),
        repeat,
        lexicon
    );
    md += "| Moteur | Calcul | Chargement | p50 | p95 | Termes conservés | Sorties suspectes | Mémoire | CPU | Mémoire vidéo |\n|---|---|---|---|---|---|---|---|---|---|\n";
    for r in &runs {
        let mut all: Vec<u32> = r.outs.iter().flat_map(|o| o.ms.clone()).collect();
        all.sort_unstable();
        let (ok, total) = lines
            .iter()
            .zip(&r.outs)
            .map(|(l, o)| kept(l, &o.text))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let sus = lines
            .iter()
            .zip(&r.outs)
            .filter(|(l, o)| suspicious(l, o).is_some())
            .count();
        md += &format!(
            "| {} | {} | {} ms | {} ms | {} ms | {ok}/{total} | {sus}/{} | {} | {} | {} |\n",
            r.name,
            r.backend,
            r.load_ms,
            pct(&all, 0.5),
            pct(&all, 0.95),
            lines.len(),
            r.rss_mb.map_or("—".into(), |m| format!("{m} Mo")),
            r.cpu_pct
                .map_or("—".into(), |c| format!("{c:.0} % d'un cœur")),
            r.vram_mb.map_or("—".into(), |m| format!("{m} Mo")),
        );
    }
    md += "\n## Lignes\n\n| id | catégorie | texte | référence |";
    for r in &runs {
        md += &format!(" {} |", r.name);
    }
    md += "\n|---|---|---|---|";
    for _ in &runs {
        md += "---|";
    }
    md += "\n";
    for (i, l) in lines.iter().enumerate() {
        md += &format!(
            "| {} | {}{} | {} | {} |",
            l.id,
            l.cat,
            if callouts::translate(&l.lang, &l.text).is_some() {
                " · lexique"
            } else {
                ""
            },
            cell(&l.text),
            cell(&l.reference)
        );
        for r in &runs {
            let o = &r.outs[i];
            let (k, n) = kept(l, &o.text);
            let flag = suspicious(l, o)
                .map(|s| format!(" ⚠ {s}"))
                .unwrap_or_default();
            let median = {
                let mut m = o.ms.clone();
                m.sort_unstable();
                pct(&m, 0.5)
            };
            md += &format!(
                " {}{} ({k}/{n} termes, {median} ms) |",
                cell(o.error.as_deref().map_or(&o.text, |e| e)),
                flag
            );
        }
        md += "\n";
    }
    if lines.iter().any(|l| l.whisper.is_some()) {
        md += "\n## Transcription ou traduction ?\n\nPour chaque ligne enregistrée : termes conservés en traduisant le texte exact, puis le texte de Whisper. Exact bon et Whisper mauvais : l'erreur vient de la transcription ; les deux mauvais : de la traduction.\n\n| id | Whisper |";
        for r in &runs {
            md += &format!(" {} (exact → Whisper) | origine |", r.name);
        }
        md += "\n|---|---|";
        for _ in &runs {
            md += "---|---|";
        }
        md += "\n";
        for (i, l) in lines.iter().enumerate() {
            let Some(w) = &l.whisper else { continue };
            md += &format!("| {} | {} |", l.id, cell(w));
            for r in &runs {
                let exact = kept(l, &r.outs[i].text);
                let heard = r.whisper_outs[i]
                    .as_ref()
                    .map_or((0, exact.1), |o| kept(l, &o.text));
                let origin = match (exact.0 == exact.1, heard.0 == heard.1) {
                    (true, true) => "—",
                    (true, false) => "transcription",
                    (false, _) => "traduction",
                };
                md += &format!(
                    " {}/{} → {}/{} : {} | {origin} |",
                    exact.0,
                    exact.1,
                    heard.0,
                    heard.1,
                    cell(r.whisper_outs[i].as_ref().map_or("", |o| &o.text))
                );
            }
            md += "\n";
        }
    }
    md += "\nContrôles automatiques : termes conservés, longueur, absence de commentaire et de cyrillique. Le sens se juge à la lecture.\n";
    print!("{md}");
    if let Some(out) = arg(&args, "--markdown") {
        std::fs::write(&out, &md).unwrap_or_else(|e| panic!("{out} : {e}"));
        eprintln!("rapport : {out}");
    }
}
