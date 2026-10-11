//! Tauri commands exposed to the UI.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    /// Build target, e.g. `windows-x86_64`.
    pub target: String,
}

impl AppInfo {
    pub fn current() -> Self {
        Self {
            name: "Crosstalk".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            target: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        }
    }
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo::current()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_crate_version() {
        let info = app_info();
        assert_eq!(info.name, "Crosstalk");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.target.contains(std::env::consts::ARCH));
    }

    #[test]
    fn app_info_serialises_for_the_ui() {
        let json = serde_json::to_value(AppInfo::current()).unwrap();
        assert!(json.get("name").is_some());
        assert!(json.get("version").is_some());
        assert!(json.get("target").is_some());
    }
}
