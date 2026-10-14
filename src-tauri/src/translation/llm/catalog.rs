//! Local LLM translation models (GGUF files run by llama.cpp in the
//! `crosstalk-llm` helper). Also compiled into the installer (src-installer):
//! std, serde and serde_json only.
//!
//! TranslateGemma 4B (Google, Gemma Terms of Use): the GGUF conversion of
//! `google/translategemma-4b-it` published by mradermacher (static quants,
//! not gated, unlike Google's repository). Q4_K_M: 2.49 GB, the usual
//! quality / size compromise for a 4B model on a gaming PC.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptFormat {
    /// TranslateGemma's chat template (chat_template.jinja of the model).
    TranslateGemma,
}

#[derive(Debug)]
pub struct LlmSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub quant: &'static str,
    pub base_model: &'static str,
    pub repo: &'static str,
    /// Git revision of `repo` the file is fetched from.
    pub revision: &'static str,
    pub file: &'static str,
    pub dir: &'static str,
    /// Exact size on Hugging Face: a different size is an interrupted download.
    pub bytes: u64,
    /// SHA-256 of the file when pinned here. `None`: the SHA-256 Hugging Face
    /// announces for the file (Git LFS object id, `X-Linked-Etag`) is checked
    /// against the downloaded bytes, then recorded in the manifest.
    pub sha256: Option<&'static str>,
    pub license: &'static str,
    pub license_url: &'static str,
    pub prompt: PromptFormat,
    /// Context of the helper (tokens): a callout prompt is ~100 tokens.
    pub ctx: u32,
    /// Memory once loaded (MB, weights + context), shown before choosing.
    pub ram_mb: u32,
}

pub const TRANSLATEGEMMA_4B: LlmSpec = LlmSpec {
    id: "translategemma-4b-q4km",
    name: "TranslateGemma 4B",
    quant: "Q4_K_M",
    base_model: "google/translategemma-4b-it",
    repo: "mradermacher/translategemma-4b-it-GGUF",
    revision: "main",
    file: "translategemma-4b-it.Q4_K_M.gguf",
    dir: "translategemma-4b-it",
    bytes: 2_489_909_760,
    sha256: None,
    license: "Gemma Terms of Use",
    license_url: "https://ai.google.dev/gemma/terms",
    prompt: PromptFormat::TranslateGemma,
    ctx: 1024,
    ram_mb: 3_300,
};

pub const CATALOG: &[LlmSpec] = &[TRANSLATEGEMMA_4B];

/// The model the app uses.
pub fn default_model() -> &'static LlmSpec {
    &CATALOG[0]
}

pub fn spec(id: &str) -> Option<&'static LlmSpec> {
    CATALOG.iter().find(|m| m.id == id)
}

pub fn url(m: &LlmSpec) -> String {
    format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        m.repo, m.revision, m.file
    )
}

pub fn model_path(models_dir: &Path, m: &LlmSpec) -> PathBuf {
    models_dir.join(m.dir).join(m.file)
}

/// Written next to the model once its SHA-256 has been checked: the app and
/// the installer trust a file of the right size only with this manifest.
pub fn manifest_path(models_dir: &Path, m: &LlmSpec) -> PathBuf {
    models_dir.join(m.dir).join("crosstalk-manifest.json")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub model: String,
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    pub source: String,
    /// Seconds since 1970 (UTC).
    pub verified_at: u64,
}

pub fn read_manifest(models_dir: &Path, m: &LlmSpec) -> Option<Manifest> {
    let raw = std::fs::read(manifest_path(models_dir, m)).ok()?;
    serde_json::from_slice(&raw).ok()
}

pub fn write_manifest(path: &Path, manifest: &Manifest) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("manifeste : {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("manifeste : {e}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum Install {
    Missing,
    /// Download started (bytes on disk).
    Partial {
        bytes: u64,
    },
    /// Right size but never checked (copied by hand, interrupted check).
    Unverified,
    Verified {
        sha256: String,
    },
    /// Wrong size or a manifest that does not match the catalog.
    Corrupt,
}

/// State of a model from the files on disk, without hashing (fast: called at
/// every start). [`Install::Verified`] means size and manifest agree.
pub fn install_state(models_dir: &Path, m: &LlmSpec) -> Install {
    let path = model_path(models_dir, m);
    let Ok(meta) = std::fs::metadata(&path) else {
        let mut part = path.into_os_string();
        part.push(".part");
        return match std::fs::metadata(PathBuf::from(part)) {
            Ok(p) => Install::Partial { bytes: p.len() },
            Err(_) => Install::Missing,
        };
    };
    if meta.len() != m.bytes {
        return Install::Corrupt;
    }
    match read_manifest(models_dir, m) {
        None => Install::Unverified,
        Some(man) if manifest_matches(&man, m) => Install::Verified { sha256: man.sha256 },
        Some(_) => Install::Corrupt,
    }
}

pub fn manifest_matches(man: &Manifest, m: &LlmSpec) -> bool {
    man.file == m.file
        && man.bytes == m.bytes
        && is_sha256(&man.sha256)
        && m.sha256
            .map_or(true, |pinned| pinned.eq_ignore_ascii_case(&man.sha256))
}

pub fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("crosstalk-llm-cat-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn catalog_entries_are_consistent() {
        for m in CATALOG {
            assert!(m.file.ends_with(".gguf"));
            assert!(m.bytes > 1_000_000_000);
            assert!(m.sha256.map_or(true, is_sha256));
            assert!(url(m).starts_with("https://huggingface.co/"));
            assert!(url(m).ends_with(m.file));
            assert!(spec(m.id).is_some());
        }
        assert_eq!(default_model().name, "TranslateGemma 4B");
    }

    #[test]
    fn install_state_follows_the_files() {
        let dir = tmp("state");
        let m = &TRANSLATEGEMMA_4B;
        assert_eq!(install_state(&dir, m), Install::Missing);
        std::fs::create_dir_all(dir.join(m.dir)).unwrap();
        let path = model_path(&dir, m);
        let mut part = path.clone().into_os_string();
        part.push(".part");
        std::fs::write(PathBuf::from(&part), b"abc").unwrap();
        assert_eq!(install_state(&dir, m), Install::Partial { bytes: 3 });
        // A sparse file of the exact size stands for the model.
        std::fs::File::create(&path)
            .unwrap()
            .set_len(m.bytes)
            .unwrap();
        assert_eq!(install_state(&dir, m), Install::Unverified);
        let mut man = Manifest {
            model: m.id.into(),
            file: m.file.into(),
            bytes: m.bytes,
            sha256: "ab".repeat(32),
            source: url(m),
            verified_at: 0,
        };
        write_manifest(&manifest_path(&dir, m), &man).unwrap();
        assert_eq!(
            install_state(&dir, m),
            Install::Verified {
                sha256: "ab".repeat(32)
            }
        );
        man.bytes = 1;
        write_manifest(&manifest_path(&dir, m), &man).unwrap();
        assert_eq!(install_state(&dir, m), Install::Corrupt);
        std::fs::File::create(&path).unwrap().set_len(10).unwrap();
        assert_eq!(install_state(&dir, m), Install::Corrupt);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_pinned_hash_must_match_the_manifest() {
        let pinned = LlmSpec {
            sha256: Some("CD".repeat(32).leak()),
            ..TRANSLATEGEMMA_4B
        };
        let man = Manifest {
            model: pinned.id.into(),
            file: pinned.file.into(),
            bytes: pinned.bytes,
            sha256: "cd".repeat(32),
            source: String::new(),
            verified_at: 0,
        };
        assert!(manifest_matches(&man, &pinned));
        let other = Manifest {
            sha256: "ef".repeat(32),
            ..man.clone()
        };
        assert!(!manifest_matches(&other, &pinned));
        assert!(manifest_matches(&other, &TRANSLATEGEMMA_4B));
        assert!(!is_sha256("xyz"));
    }
}
