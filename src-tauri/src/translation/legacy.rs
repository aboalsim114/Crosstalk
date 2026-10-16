//! Opus-MT models left on disk by earlier versions (`<models>/opus-mt-*`).
//! Never deleted behind the user's back: Audio & IA shows their size and a
//! button removes them.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyModels {
    pub folders: usize,
    pub bytes: u64,
}

fn folders(models: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(models) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_string_lossy().starts_with("opus-mt-"))
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

fn size(path: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(path) else {
        return 0;
    };
    rd.filter_map(Result::ok)
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0, // links are not followed
        })
        .sum()
}

pub fn scan(models: &Path) -> LegacyModels {
    let f = folders(models);
    LegacyModels {
        bytes: f.iter().map(|p| size(p)).sum(),
        folders: f.len(),
    }
}

/// Removes the old folders (and only them). Returns what is left (nothing on success).
pub fn remove(models: &Path) -> Result<LegacyModels, String> {
    for f in folders(models) {
        std::fs::remove_dir_all(&f).map_err(|e| format!("{} : {e}", f.display()))?;
    }
    Ok(scan(models))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_removes_only_the_old_opus_mt_folders() {
        let d = std::env::temp_dir().join(format!("crosstalk-legacy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("opus-mt-ru-fr")).unwrap();
        std::fs::create_dir_all(d.join("opus-mt-en-fr/sub")).unwrap();
        std::fs::write(d.join("opus-mt-ru-fr/model.safetensors"), vec![0u8; 1000]).unwrap();
        std::fs::write(d.join("opus-mt-en-fr/sub/vocab.json"), vec![0u8; 24]).unwrap();
        std::fs::write(d.join("ggml-small.bin"), vec![0u8; 50]).unwrap();
        std::fs::create_dir_all(d.join("voice")).unwrap();
        assert_eq!(
            scan(&d),
            LegacyModels {
                folders: 2,
                bytes: 1024
            }
        );
        assert_eq!(
            remove(&d).unwrap(),
            LegacyModels {
                folders: 0,
                bytes: 0
            }
        );
        // Whisper and the voices stay.
        assert!(d.join("ggml-small.bin").is_file() && d.join("voice").is_dir());
        assert_eq!(
            scan(&d.join("absent")),
            LegacyModels {
                folders: 0,
                bytes: 0
            }
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
