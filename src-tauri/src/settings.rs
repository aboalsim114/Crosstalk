//! User settings on disk (cahier des charges §6.3):
//!
//! - `%APPDATA%\Crosstalk\config.json`: settings, versioned (the UI migrates
//!   older versions, see `src/app/state/persist.ts`);
//! - `%APPDATA%\Crosstalk\glossary.json`: the user's glossary.
//!
//! Written atomically (temporary file then rename). A file that cannot be
//! parsed is kept aside as `*.bak` and the defaults apply, with a warning.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

const CONFIG: &str = "config.json";
const GLOSSARY: &str = "glossary.json";

/// `%APPDATA%\Crosstalk` (or `CROSSTALK_CONFIG_DIR`).
pub fn dir() -> PathBuf {
    if let Some(d) = std::env::var_os("CROSSTALK_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Crosstalk")
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored {
    pub config: Option<Value>,
    pub glossary: Option<Value>,
    /// Files that could not be read (kept as `.bak`).
    pub warnings: Vec<String>,
    pub dir: String,
}

fn read(path: &Path, warnings: &mut Vec<String>) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Value>(&text) {
        Ok(v) if v.is_object() => Some(v),
        Ok(_) | Err(_) => {
            let bak = path.with_extension("json.bak");
            let _ = std::fs::rename(path, &bak);
            warnings.push(format!(
                "{} illisible : réglages par défaut (copie gardée dans {})",
                path.display(),
                bak.display()
            ));
            None
        }
    }
}

pub fn load(dir: &Path) -> Stored {
    let mut warnings = Vec::new();
    Stored {
        config: read(&dir.join(CONFIG), &mut warnings),
        glossary: read(&dir.join(GLOSSARY), &mut warnings),
        warnings,
        dir: dir.display().to_string(),
    }
}

fn write(path: &Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{} : {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{} : {e}", path.display()))
}

pub fn save(dir: &Path, config: &Value, glossary: &Value) -> Result<(), String> {
    if !config.is_object() || !glossary.is_object() {
        return Err("réglages invalides".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    write(&dir.join(CONFIG), config)?;
    write(&dir.join(GLOSSARY), glossary)
}

#[tauri::command]
pub fn settings_load() -> Stored {
    load(&dir())
}

#[tauri::command]
pub fn settings_save(config: Value, glossary: Value) -> Result<(), String> {
    save(&dir(), &config, &glossary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tmp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("crosstalk-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn round_trip_and_first_launch() {
        let d = tmp("rt");
        assert_eq!(
            load(&d),
            Stored {
                dir: d.display().to_string(),
                ..Default::default()
            }
        );
        let config = json!({"version": 1, "settings": {"model": "small", "ownName": "Иван"}});
        let glossary = json!({"version": 1, "terms": [["smoke", "Conserver", "smoke"]]});
        save(&d, &config, &glossary).unwrap();
        let st = load(&d);
        assert_eq!((st.config, st.glossary), (Some(config), Some(glossary)));
        assert!(st.warnings.is_empty());
        assert!(!d.join("config.json.tmp").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_broken_file_is_kept_aside() {
        let d = tmp("bad");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), "{ not json").unwrap();
        std::fs::write(d.join("glossary.json"), "[1, 2]").unwrap();
        let st = load(&d);
        assert_eq!((st.config.is_none(), st.glossary.is_none()), (true, true));
        assert_eq!(st.warnings.len(), 2);
        assert!(d.join("config.json.bak").exists() && !d.join("config.json").exists());
        assert!(save(&d, &json!([]), &json!({})).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
