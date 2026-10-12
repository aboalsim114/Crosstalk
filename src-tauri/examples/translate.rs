//! crosstalk-translate: local translation test (Opus-MT, CPU, no network).
//!
//!   crosstalk-translate --lang ru "Двое идут на B"
//!   crosstalk-translate --lang en --repeat 20 "Two on B, they have an AWP"
//!   crosstalk-translate --model-dir C:\models\opus-mt-ru-fr --max-new 24 "…"
//!
//! Models: `scripts/download-models.ps1 -Translation` (or CROSSTALK_MODELS_DIR).
//! `--exact` uses the full f32 output layer (reference), default is the app's fast mode.
//! `--max-new` forces the number of decoding steps (timing with random weights).
use std::path::PathBuf;
use std::time::Instant;

use crosstalk_lib::translation::{
    marian::{Marian, Mode},
    models,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let text = args
        .iter()
        .enumerate()
        .rev()
        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || !args[i - 1].starts_with("--")))
        .map(|(_, a)| a.clone())
        .unwrap_or_else(|| {
            eprintln!("usage : crosstalk-translate [--lang ru|en] [--model-dir DIR] [--repeat N] [--max-new N] TEXTE");
            std::process::exit(2)
        });
    let lang = opt("--lang").unwrap_or_else(|| "ru".into());
    let dir = opt("--model-dir").map(PathBuf::from).unwrap_or_else(|| {
        let pair = models::pair(&lang).unwrap_or_else(|| {
            eprintln!("langue non gérée : {lang} (ru ou en)");
            std::process::exit(2)
        });
        models::pair_dir(&crosstalk_lib::speech::models::models_dir(), pair)
    });
    let repeat: usize = opt("--repeat")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);
    let forced: Option<usize> = opt("--max-new").and_then(|v| v.parse().ok());

    let t0 = Instant::now();
    let mode = if args.iter().any(|a| a == "--exact") {
        Mode::Exact
    } else {
        Mode::Fast
    };
    let mut m = Marian::load(&dir, mode).unwrap_or_else(|e| {
        eprintln!("erreur : {e} ({})", dir.display());
        std::process::exit(2)
    });
    println!(
        "modèle {} chargé en {} ms",
        dir.display(),
        t0.elapsed().as_millis()
    );
    let src = m.encode(&text);
    let mut times = Vec::new();
    let mut out = String::new();
    for _ in 0..repeat {
        let t = Instant::now();
        out = match forced {
            Some(n) => {
                let ids = m.generate(&src, n).expect("génération");
                format!("({} jetons) {}", ids.len(), m.decode(&ids))
            }
            None => m.translate(&text).expect("traduction"),
        };
        times.push(t.elapsed().as_millis() as u64);
    }
    times.sort_unstable();
    println!("{} jetons source", src.len());
    println!("FR : {out}");
    println!(
        "temps : p50 {} ms · max {} ms ({} essais)",
        times[times.len() / 2],
        times[times.len() - 1],
        times.len()
    );
}
