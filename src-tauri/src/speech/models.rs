//! Speech model catalog and the files expected on disk.
//!
//! Models are not bundled with the installer (hundreds of MB). They live in
//! `crate::paths::models_dir()` and are downloaded by the installer or the app
//! (`crate::downloads`), checked by exact size and SHA-256.
//!
//! Recognition: NVIDIA Parakeet TDT 0.6B v3 (CC-BY-4.0), int8 ONNX export of
//! the sherpa-onnx project, run by sherpa-onnx / ONNX Runtime on the CPU.
//! Voice activity: Silero VAD v5 (MIT), ONNX. See
//! docs/reports/asr-engine-replacement.md for the comparison that chose them.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub const DEFAULT_MODEL: &str = "parakeet-tdt-0.6b-v3";

/// Silero VAD v5 (ONNX), as published with the sherpa-onnx models.
pub const VAD_FILE: &str = "silero_vad.onnx";
pub const VAD_BYTES: u64 = 643_854;
pub const VAD_SHA256: &str = "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6";
pub const VAD_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx";

/// How sherpa-onnx runs the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    /// NeMo transducer (encoder, decoder, joiner, tokens).
    NemoTransducer,
}

pub struct ModelFile {
    pub name: &'static str,
    /// Exact published size: a shorter file is an interrupted download.
    pub bytes: u64,
    pub sha256: &'static str,
}

pub struct ModelSpec {
    pub id: &'static str,
    /// Shown in the interface.
    pub label: &'static str,
    /// Folder inside the models directory.
    pub dir: &'static str,
    pub kind: ModelKind,
    /// Base download address; each file is `<base>/<name>`.
    pub base_url: &'static str,
    pub files: &'static [ModelFile],
    pub note: &'static str,
    pub license: &'static str,
}

impl ModelSpec {
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.bytes).sum()
    }

    pub fn size_mb(&self) -> u32 {
        (self.bytes() / 1_000_000) as u32
    }

    pub fn url(&self, file: &ModelFile) -> String {
        format!("{}/{}", self.base_url, file.name)
    }
}

pub const CATALOG: &[ModelSpec] = &[ModelSpec {
    id: "parakeet-tdt-0.6b-v3",
    label: "Parakeet TDT 0.6B v3",
    dir: "parakeet-tdt-0.6b-v3-int8",
    kind: ModelKind::NemoTransducer,
    base_url:
        "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/main",
    files: &[
        ModelFile {
            name: "encoder.int8.onnx",
            bytes: 652_184_281,
            sha256: "acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247",
        },
        ModelFile {
            name: "decoder.int8.onnx",
            bytes: 11_845_275,
            sha256: "179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e",
        },
        ModelFile {
            name: "joiner.int8.onnx",
            bytes: 6_355_277,
            sha256: "3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3",
        },
        ModelFile {
            name: "tokens.txt",
            bytes: 93_939,
            sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d",
        },
    ],
    note: "Russe, anglais, français et 22 autres langues · processeur",
    license: "CC-BY-4.0 (NVIDIA)",
}];

pub fn spec(id: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|m| m.id == id)
}

/// The catalog entry for `id`, or the default one (settings of an older
/// version name a Whisper model that no longer exists).
pub fn spec_or_default(id: &str) -> &'static ModelSpec {
    spec(id).unwrap_or(&CATALOG[0])
}

/// A file is usable if it exists with its exact published size (its SHA-256
/// is checked once, by the download).
fn installed(path: &Path, bytes: u64) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() == bytes)
        .unwrap_or(false)
}

/// The model's folder, when every file is in place.
pub fn model_path(dir: &Path, id: &str) -> Option<PathBuf> {
    let spec = spec(id)?;
    let folder = dir.join(spec.dir);
    spec.files
        .iter()
        .all(|f| installed(&folder.join(f.name), f.bytes))
        .then_some(folder)
}

pub fn vad_path(dir: &Path) -> Option<PathBuf> {
    let path = dir.join(VAD_FILE);
    installed(&path, VAD_BYTES).then_some(path)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub size_mb: u32,
    pub note: String,
    pub license: String,
    pub installed: bool,
}

pub fn list(dir: &Path) -> Vec<ModelInfo> {
    CATALOG
        .iter()
        .map(|m| ModelInfo {
            id: m.id.into(),
            label: m.label.into(),
            size_mb: m.size_mb(),
            note: m.note.into(),
            license: m.license.into(),
            installed: model_path(dir, m.id).is_some(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offline_download_script_has_the_same_files() {
        let script = include_str!("../../../scripts/download-models.ps1");
        assert!(script.contains(VAD_SHA256) && script.contains(VAD_URL));
        for m in CATALOG {
            assert!(script.contains(m.base_url), "{}", m.id);
            for f in m.files {
                assert!(script.contains(f.sha256), "{}", f.name);
                assert!(
                    script.contains(&format!("Size = {};", f.bytes)),
                    "{}",
                    f.name
                );
                assert!(
                    script.contains(&format!("{}\\{}", m.dir, f.name)),
                    "{}",
                    f.name
                );
            }
        }
        assert!(!script.to_lowercase().contains("whisper"));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("crosstalk-models-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn catalog_has_the_default_and_checksums() {
        assert!(spec(DEFAULT_MODEL).is_some());
        assert_eq!(
            spec_or_default("small").id,
            DEFAULT_MODEL,
            "old Whisper ids"
        );
        for m in CATALOG {
            assert!(m.files.iter().all(|f| f.sha256.len() == 64 && f.bytes > 0));
            assert!(m.base_url.starts_with("https://"));
        }
        let p = spec(DEFAULT_MODEL).unwrap();
        assert!(p.size_mb() > 600);
        assert_eq!(
            p.url(&p.files[3]),
            "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/main/tokens.txt"
        );
        assert_eq!(VAD_SHA256.len(), 64);
    }

    #[test]
    fn detects_installed_and_truncated_files() {
        let d = temp_dir("detect");
        let p = spec(DEFAULT_MODEL).unwrap();
        assert!(model_path(&d, p.id).is_none());
        assert!(vad_path(&d).is_none());
        let folder = d.join(p.dir);
        std::fs::create_dir_all(&folder).unwrap();
        for f in p.files {
            std::fs::File::create(folder.join(f.name))
                .unwrap()
                .set_len(f.bytes)
                .unwrap();
        }
        assert_eq!(model_path(&d, p.id).as_deref(), Some(folder.as_path()));
        // One truncated file: not installed.
        std::fs::File::create(folder.join("tokens.txt"))
            .unwrap()
            .set_len(10)
            .unwrap();
        assert!(model_path(&d, p.id).is_none());
        std::fs::File::create(d.join(VAD_FILE))
            .unwrap()
            .set_len(VAD_BYTES)
            .unwrap();
        assert!(vad_path(&d).is_some());
        assert!(!list(&d)[0].installed);
        assert!(model_path(&d, "unknown").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
