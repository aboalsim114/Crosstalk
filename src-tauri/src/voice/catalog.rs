//! What "Voix IA" downloads: the Piper engine (Windows build, MIT) and the
//! neural voices (rhasspy/piper-voices on Hugging Face). Sizes are the
//! published ones; the engine is also checked by SHA-1.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::downloads::FileSpec;

/// Piper 2023.11.14-2 for Windows x64: piper.exe, onnxruntime, espeak-ng data.
pub const ENGINE_URL: &str =
    "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip";
pub const ENGINE_BYTES: u64 = 22_477_236;
pub const ENGINE_SHA1: &str = "48b7ae8a46c07124aacbd112a1ba0ed4a913e3f0";
const ENGINE_ZIP: &str = "piper_windows_amd64.zip";
const VOICES_BASE: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Gender {
    Female,
    Male,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    /// Piper name, e.g. `ru_RU-irina-medium`.
    pub id: &'static str,
    /// Spoken language, as elsewhere in the app (`ru`, `en`…).
    pub lang: &'static str,
    /// Shown in the list.
    pub name: &'static str,
    pub gender: Gender,
    /// Size of the `.onnx` model, then of its `.onnx.json`.
    pub onnx_bytes: u64,
    pub json_bytes: u64,
}

const fn v(
    id: &'static str,
    lang: &'static str,
    name: &'static str,
    gender: Gender,
    onnx_bytes: u64,
    json_bytes: u64,
) -> Voice {
    Voice {
        id,
        lang,
        name,
        gender,
        onnx_bytes,
        json_bytes,
    }
}

/// Two voices per language when Piper has a good one of each.
pub const VOICES: &[Voice] = &[
    v(
        "ru_RU-irina-medium",
        "ru",
        "Irina",
        Gender::Female,
        63_201_294,
        4_765,
    ),
    v(
        "ru_RU-dmitri-medium",
        "ru",
        "Dmitri",
        Gender::Male,
        63_201_294,
        4_824,
    ),
    v(
        "en_US-amy-medium",
        "en",
        "Amy",
        Gender::Female,
        63_201_294,
        4_882,
    ),
    v(
        "en_US-ryan-medium",
        "en",
        "Ryan",
        Gender::Male,
        63_201_294,
        4_883,
    ),
    v(
        "fr_FR-siwis-medium",
        "fr",
        "Siwis",
        Gender::Female,
        63_201_294,
        4_875,
    ),
    v(
        "fr_FR-tom-medium",
        "fr",
        "Tom",
        Gender::Male,
        63_511_038,
        4_959,
    ),
    v(
        "de_DE-thorsten-medium",
        "de",
        "Thorsten",
        Gender::Male,
        63_201_294,
        4_819,
    ),
    v(
        "es_ES-davefx-medium",
        "es",
        "Davefx",
        Gender::Male,
        63_201_294,
        4_817,
    ),
    v(
        "pl_PL-gosia-medium",
        "pl",
        "Gosia",
        Gender::Female,
        63_201_294,
        4_814,
    ),
    v(
        "pl_PL-darkman-medium",
        "pl",
        "Darkman",
        Gender::Male,
        63_201_294,
        4_816,
    ),
    v(
        "tr_TR-dfki-medium",
        "tr",
        "Dfki",
        Gender::Male,
        63_201_294,
        4_960,
    ),
    v(
        "uk_UA-lada-x_low",
        "uk",
        "Lada",
        Gender::Female,
        20_628_813,
        4_186,
    ),
];

pub fn voice(id: &str) -> Option<&'static Voice> {
    VOICES.iter().find(|v| v.id == id)
}

/// The first voice of a language (the default one).
pub fn default_voice(lang: &str) -> Option<&'static Voice> {
    VOICES.iter().find(|v| v.lang == lang)
}

/// `ru_RU-irina-medium` → `ru/ru_RU/irina/medium/ru_RU-irina-medium`.
fn hf_path(id: &str) -> Option<String> {
    let mut parts = id.splitn(3, '-');
    let (locale, name, quality) = (parts.next()?, parts.next()?, parts.next()?);
    let lang = locale.split('_').next()?;
    Some(format!("{lang}/{locale}/{name}/{quality}/{id}"))
}

/// Everything lives in `<models>/voice`.
pub fn dir(models: &Path) -> PathBuf {
    models.join("voice")
}

pub fn engine_zip(models: &Path) -> PathBuf {
    dir(models).join(ENGINE_ZIP)
}

/// piper.exe once the zip is extracted (the archive has a `piper/` folder).
pub fn engine_exe(models: &Path) -> PathBuf {
    dir(models).join("piper").join("piper.exe")
}

pub fn model_path(models: &Path, v: &Voice) -> PathBuf {
    dir(models).join("voices").join(format!("{}.onnx", v.id))
}

/// Piper reads the voice settings next to the model (`model + .json`).
pub fn config_path(models: &Path, v: &Voice) -> PathBuf {
    dir(models)
        .join("voices")
        .join(format!("{}.onnx.json", v.id))
}

fn has_size(path: &Path, bytes: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.len() == bytes)
}

/// The engine is ready (extracted), or at least its archive is complete.
pub fn engine_ready(models: &Path) -> bool {
    engine_exe(models).is_file() || has_size(&engine_zip(models), ENGINE_BYTES)
}

pub fn voice_installed(models: &Path, v: &Voice) -> bool {
    has_size(&model_path(models, v), v.onnx_bytes)
        && has_size(&config_path(models, v), v.json_bytes)
}

/// Files still to download for these voices (and the engine when needed).
pub fn missing(models: &Path, voice_ids: &[String]) -> Vec<FileSpec> {
    let mut out = Vec::new();
    if !engine_ready(models) {
        out.push(FileSpec {
            label: "Moteur de voix (Piper)".into(),
            url: ENGINE_URL.into(),
            path: engine_zip(models),
            bytes: ENGINE_BYTES,
            sha1: Some(ENGINE_SHA1),
        });
    }
    for v in voice_ids.iter().filter_map(|id| voice(id)) {
        let Some(base) = hf_path(v.id) else { continue };
        for (path, bytes, ext) in [
            (model_path(models, v), v.onnx_bytes, "onnx"),
            (config_path(models, v), v.json_bytes, "onnx.json"),
        ] {
            if !has_size(&path, bytes) {
                out.push(FileSpec {
                    label: format!("Voix {} ({})", v.name, v.lang.to_uppercase()),
                    url: format!("{VOICES_BASE}/{base}.{ext}"),
                    path,
                    bytes,
                    sha1: None,
                });
            }
        }
    }
    out
}

/// Extracts the engine archive (once). Only `piper/…` entries, never outside
/// the voice folder.
pub fn extract_engine(models: &Path) -> Result<PathBuf, String> {
    let exe = engine_exe(models);
    if exe.is_file() {
        return Ok(exe);
    }
    let zip = engine_zip(models);
    let file =
        std::fs::File::open(&zip).map_err(|_| "moteur de voix non téléchargé".to_string())?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("archive du moteur illisible : {e}"))?;
    let root = dir(models);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        if !rel.starts_with("piper") {
            continue;
        }
        let out = root.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut w).map_err(|e| e.to_string())?;
    }
    if exe.is_file() {
        Ok(exe)
    } else {
        Err("piper.exe absent de l'archive".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reply_language_has_a_voice_with_a_valid_path() {
        for lang in ["ru", "en", "fr", "de", "es", "pl", "tr", "uk"] {
            assert!(default_voice(lang).is_some(), "{lang}");
        }
        for v in VOICES {
            assert!(v.id.starts_with(&format!("{}_", v.lang)), "{}", v.id);
            assert!(v.onnx_bytes > 10_000_000 && v.json_bytes > 1_000);
        }
        assert_eq!(
            hf_path("ru_RU-irina-medium").as_deref(),
            Some("ru/ru_RU/irina/medium/ru_RU-irina-medium")
        );
        assert_eq!(
            hf_path("uk_UA-lada-x_low").as_deref(),
            Some("uk/uk_UA/lada/x_low/uk_UA-lada-x_low")
        );
    }

    #[test]
    fn lists_the_engine_and_both_files_of_each_voice_until_present() {
        let d = std::env::temp_dir().join(format!("crosstalk-voice-cat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let todo = missing(&d, &["ru_RU-irina-medium".into(), "nope".into()]);
        assert_eq!(todo.len(), 3);
        assert_eq!(todo[0].sha1, Some(ENGINE_SHA1));
        assert!(todo[1]
            .url
            .ends_with("/ru/ru_RU/irina/medium/ru_RU-irina-medium.onnx"));
        assert!(todo[2].url.ends_with(".onnx.json"));
        // Once the files have their size, nothing is left.
        let v = voice("ru_RU-irina-medium").unwrap();
        std::fs::create_dir_all(dir(&d).join("voices")).unwrap();
        std::fs::File::create(model_path(&d, v))
            .unwrap()
            .set_len(v.onnx_bytes)
            .unwrap();
        std::fs::File::create(config_path(&d, v))
            .unwrap()
            .set_len(v.json_bytes)
            .unwrap();
        std::fs::File::create(engine_zip(&d))
            .unwrap()
            .set_len(ENGINE_BYTES)
            .unwrap();
        assert!(voice_installed(&d, v));
        assert!(missing(&d, &["ru_RU-irina-medium".into()]).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The real archive, when given: `CROSSTALK_PIPER_ZIP=/path/piper_windows_amd64.zip`.
    #[test]
    fn extracts_the_real_engine_archive() {
        let Some(zip) = std::env::var_os("CROSSTALK_PIPER_ZIP") else {
            eprintln!("CROSSTALK_PIPER_ZIP non défini : test réel ignoré");
            return;
        };
        let d = std::env::temp_dir().join(format!("crosstalk-voice-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(dir(&d)).unwrap();
        std::fs::copy(zip, engine_zip(&d)).unwrap();
        assert!(engine_ready(&d));
        let exe = extract_engine(&d).unwrap();
        assert!(exe.is_file());
        assert!(dir(&d).join("piper").join("espeak-ng-data").is_dir());
        assert!(dir(&d).join("piper").join("onnxruntime.dll").is_file());
        // A second call does nothing.
        assert_eq!(extract_engine(&d).unwrap(), exe);
        let _ = std::fs::remove_dir_all(&d);
    }
}
