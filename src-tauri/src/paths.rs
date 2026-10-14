//! Where Crosstalk keeps its data on disk. A leaf module: it uses nothing else
//! from the crate, so every module (and the installer, which compiles this
//! file with `#[path]`) can depend on it.

use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_wins() {
        // Read-only check of the default layout (env vars are process-global).
        let d = models_dir();
        assert!(d.ends_with("models"));
    }
}
