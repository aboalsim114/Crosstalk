//! Whisper files left on disk by earlier versions (`<models>/ggml-*.bin`: the
//! Whisper models and the ggml Silero VAD, plus interrupted downloads). Never
//! deleted behind the user's back: Audio & IA shows their size and a button
//! removes them.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyWhisper {
    pub files: usize,
    pub bytes: u64,
}

fn is_whisper_file(name: &str) -> bool {
    name.starts_with("ggml-") && (name.ends_with(".bin") || name.ends_with(".bin.part"))
}

fn files(models: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(models) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| is_whisper_file(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

pub fn scan(models: &Path) -> LegacyWhisper {
    let f = files(models);
    LegacyWhisper {
        bytes: f
            .iter()
            .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .sum(),
        files: f.len(),
    }
}

/// Removes the old Whisper files (and only them). Returns what is left
/// (nothing on success).
pub fn remove(models: &Path) -> Result<LegacyWhisper, String> {
    for f in files(models) {
        std::fs::remove_file(&f).map_err(|e| format!("{} : {e}", f.display()))?;
    }
    Ok(scan(models))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_removes_only_the_old_whisper_files() {
        let d = std::env::temp_dir().join(format!("crosstalk-whisper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("parakeet-tdt-0.6b-v3-int8")).unwrap();
        std::fs::write(d.join("ggml-small.bin"), vec![0u8; 1000]).unwrap();
        std::fs::write(d.join("ggml-silero-v5.1.2.bin"), vec![0u8; 20]).unwrap();
        std::fs::write(d.join("ggml-medium.bin.part"), vec![0u8; 4]).unwrap();
        std::fs::write(d.join("silero_vad.onnx"), vec![0u8; 8]).unwrap();
        std::fs::write(d.join("parakeet-tdt-0.6b-v3-int8/tokens.txt"), b"a").unwrap();
        assert_eq!(
            scan(&d),
            LegacyWhisper {
                files: 3,
                bytes: 1024
            }
        );
        assert_eq!(remove(&d).unwrap(), LegacyWhisper { files: 0, bytes: 0 });
        // The new model and VAD stay.
        assert!(d.join("silero_vad.onnx").is_file());
        assert!(d.join("parakeet-tdt-0.6b-v3-int8/tokens.txt").is_file());
        assert_eq!(
            scan(&d.join("absent")),
            LegacyWhisper { files: 0, bytes: 0 }
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
