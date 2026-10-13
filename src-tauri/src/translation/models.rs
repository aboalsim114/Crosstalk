//! Translation model catalog: Helsinki-NLP Opus-MT (Apache-2.0), one model per
//! source language, stored in `<models dir>/opus-mt-<src>-fr/` with the files
//! of the Hugging Face repository (`scripts/download-models.ps1 -Translation`).

use std::path::{Path, PathBuf};

use serde::Serialize;

pub struct PairSpec {
    pub source: &'static str,
    pub target: &'static str,
    pub repo: &'static str,
    pub dir: &'static str,
    /// Exact sizes on Hugging Face: a different size is an interrupted download.
    pub files: [(&'static str, u64); 5],
}

pub const CATALOG: &[PairSpec] = &[
    PairSpec {
        source: "ru",
        target: "fr",
        repo: "Helsinki-NLP/opus-mt-ru-fr",
        dir: "opus-mt-ru-fr",
        files: [
            ("config.json", 1_416),
            ("vocab.json", 2_658_919),
            ("source.spm", 1_063_848),
            ("target.spm", 820_963),
            ("pytorch_model.bin", 310_806_561),
        ],
    },
    PairSpec {
        source: "en",
        target: "fr",
        repo: "Helsinki-NLP/opus-mt-en-fr",
        dir: "opus-mt-en-fr",
        files: [
            ("config.json", 1_416),
            ("vocab.json", 1_339_166),
            ("source.spm", 778_395),
            ("target.spm", 802_397),
            ("pytorch_model.bin", 300_827_685),
        ],
    },
];

/// French to the teammates' language ("Réponds dans leur langue", src/reply).
pub const REPLY: &[PairSpec] = &[
    PairSpec {
        source: "fr",
        target: "en",
        repo: "Helsinki-NLP/opus-mt-fr-en",
        dir: "opus-mt-fr-en",
        files: [
            ("config.json", 1_416),
            ("vocab.json", 1_339_166),
            ("source.spm", 802_397),
            ("target.spm", 778_395),
            ("pytorch_model.bin", 300_827_685),
        ],
    },
    PairSpec {
        source: "fr",
        target: "ru",
        repo: "Helsinki-NLP/opus-mt-fr-ru",
        dir: "opus-mt-fr-ru",
        files: [
            ("config.json", 1_381),
            ("vocab.json", 2_658_919),
            ("source.spm", 820_963),
            ("target.spm", 1_063_848),
            ("pytorch_model.bin", 310_806_561),
        ],
    },
];

pub fn reply_pair(target: &str) -> Option<&'static PairSpec> {
    REPLY.iter().find(|p| p.target == target)
}

pub fn pair(source: &str) -> Option<&'static PairSpec> {
    CATALOG.iter().find(|p| p.source == source)
}

pub fn pair_dir(models_dir: &Path, pair: &PairSpec) -> PathBuf {
    models_dir.join(pair.dir)
}

/// The first missing or truncated file, if any.
pub fn missing_file(models_dir: &Path, pair: &PairSpec) -> Option<&'static str> {
    let dir = pair_dir(models_dir, pair);
    pair.files.iter().find_map(|&(name, bytes)| {
        let ok = std::fs::metadata(dir.join(name)).is_ok_and(|m| m.len() == bytes);
        (!ok).then_some(name)
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairInfo {
    pub source: &'static str,
    pub repo: &'static str,
    pub size_mb: u32,
    pub installed: bool,
}

pub fn list(models_dir: &Path) -> Vec<PairInfo> {
    CATALOG
        .iter()
        .map(|p| PairInfo {
            source: p.source,
            repo: p.repo,
            size_mb: (p.files.iter().map(|f| f.1).sum::<u64>() / 1_000_000) as u32,
            installed: missing_file(models_dir, p).is_none(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_truncated_file_is_reported_missing() {
        let root = std::env::temp_dir().join(format!("crosstalk-mt-{}", std::process::id()));
        let p = pair("en").unwrap();
        let dir = pair_dir(&root, p);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(missing_file(&root, p), Some("config.json"));
        std::fs::write(dir.join("config.json"), vec![b' '; 1_416]).unwrap();
        assert_eq!(missing_file(&root, p), Some("vocab.json"));
        assert!(!list(&root).iter().any(|i| i.installed));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn every_pair_targets_french() {
        for p in CATALOG {
            assert!(p.repo.ends_with(&format!("{}-fr", p.source)));
            assert_eq!(p.target, "fr");
        }
        for p in REPLY {
            assert_eq!(p.source, "fr");
            assert!(p.repo.ends_with(&format!("fr-{}", p.target)));
            assert_eq!(p.dir, p.repo.trim_start_matches("Helsinki-NLP/"));
        }
        assert!(reply_pair("ru").is_some() && reply_pair("de").is_none());
    }
}
