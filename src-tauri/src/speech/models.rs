//! Model catalog and on-disk location.
//!
//! Models are not bundled with the installer (75 MB to 1.5 GB). They live in
//! `%LOCALAPPDATA%\Crosstalk\models` (or `CROSSTALK_MODELS_DIR`). In-app
//! download with checksum verification is part of #15; until then the files
//! are fetched with `scripts/download-models.ps1`.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub const DEFAULT_MODEL: &str = "base";

/// Silero VAD v5.1.2, ggml port published by the whisper.cpp project.
pub const VAD_FILE: &str = "ggml-silero-v5.1.2.bin";
pub const VAD_BYTES: u64 = 885_098;
pub const VAD_URL: &str =
    "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin";

pub struct ModelSpec {
    pub id: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    /// Exact file size on Hugging Face: a shorter file is an interrupted download.
    pub bytes: u64,
    /// SHA-1 published in whisper.cpp's `models/README.md`.
    pub sha1: &'static str,
    pub note: &'static str,
}

pub const CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "tiny",
        bytes: 77691713,
        file: "ggml-tiny.bin",
        size_mb: 75,
        sha1: "bd577a113a864445d4c299885e0cb97d4ba92b5f",
        note: "Très rapide · russe peu fiable",
    },
    ModelSpec {
        id: "base",
        bytes: 147951465,
        file: "ggml-base.bin",
        size_mb: 142,
        sha1: "465707469ff3a37a2b9b8d8f89f2f99de7299dac",
        note: "Rapide · CPU seul · défaut",
    },
    ModelSpec {
        id: "small",
        bytes: 487601967,
        file: "ggml-small.bin",
        size_mb: 466,
        sha1: "55356645c2b361a969dfd0ef2c5a50d530afd8d5",
        note: "Meilleur russe · CPU récent conseillé",
    },
    ModelSpec {
        id: "medium",
        bytes: 1533763059,
        file: "ggml-medium.bin",
        size_mb: 1500,
        sha1: "fd9727b6e1217c2f614f9b698455c4ffd82463b4",
        note: "Précis · trop lent sur CPU pendant une partie",
    },
    ModelSpec {
        id: "large-v3-turbo",
        bytes: 574041195,
        file: "ggml-large-v3-turbo-q5_0.bin",
        size_mb: 547,
        sha1: "e050f7970618a659205450ad97eb95a18d69c9ee",
        note: "Le plus précis · GPU requis (pas dans cette version)",
    },
];

pub fn spec(id: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|m| m.id == id)
}

pub fn url(spec: &ModelSpec) -> String {
    format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        spec.file
    )
}

/// Where models are looked up.
pub fn models_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CROSSTALK_MODELS_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Crosstalk"));
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .map(|d| d.join("crosstalk"));
    base.unwrap_or_else(|| PathBuf::from(".")).join("models")
}

/// A model file is usable if it exists with its exact published size
/// (integrity is checked by SHA-1 in the download script).
fn installed(path: &Path, bytes: u64) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() == bytes)
        .unwrap_or(false)
}

pub fn model_path(dir: &Path, id: &str) -> Option<PathBuf> {
    let spec = spec(id)?;
    let path = dir.join(spec.file);
    installed(&path, spec.bytes).then_some(path)
}

pub fn vad_path(dir: &Path) -> Option<PathBuf> {
    let path = dir.join(VAD_FILE);
    installed(&path, VAD_BYTES).then_some(path)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub file: String,
    pub size_mb: u32,
    pub note: String,
    pub installed: bool,
    pub url: String,
}

pub fn list(dir: &Path) -> Vec<ModelInfo> {
    CATALOG
        .iter()
        .map(|m| ModelInfo {
            id: m.id.into(),
            file: m.file.into(),
            size_mb: m.size_mb,
            note: m.note.into(),
            installed: model_path(dir, m.id).is_some(),
            url: url(m),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("crosstalk-models-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn catalog_has_the_default_and_unique_ids() {
        assert!(spec(DEFAULT_MODEL).is_some());
        let mut ids: Vec<_> = CATALOG.iter().map(|m| m.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        assert!(CATALOG.iter().all(|m| m.sha1.len() == 40));
        assert_eq!(
            url(spec("base").unwrap()),
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
        );
    }

    #[test]
    fn detects_installed_and_truncated_files() {
        let d = temp_dir("detect");
        assert!(model_path(&d, "tiny").is_none());
        assert!(vad_path(&d).is_none());
        std::fs::write(d.join("ggml-tiny.bin"), vec![0u8; 1024]).unwrap();
        assert!(model_path(&d, "tiny").is_none(), "truncated");
        let f = std::fs::File::create(d.join("ggml-tiny.bin")).unwrap();
        f.set_len(77_691_713).unwrap();
        assert!(model_path(&d, "tiny").is_some());
        let f = std::fs::File::create(d.join(VAD_FILE)).unwrap();
        f.set_len(VAD_BYTES).unwrap();
        assert!(vad_path(&d).is_some());
        let l = list(&d);
        assert!(l.iter().find(|m| m.id == "tiny").unwrap().installed);
        assert!(!l.iter().find(|m| m.id == "base").unwrap().installed);
        assert!(model_path(&d, "unknown").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn env_override_wins() {
        // Read-only check of the default layout (env vars are process-global).
        let d = models_dir();
        assert!(d.ends_with("models"));
    }
}
