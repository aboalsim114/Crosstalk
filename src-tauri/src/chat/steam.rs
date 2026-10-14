//! Finds Counter-Strike 2 on disk, only by reading Steam's own files:
//!
//! - the Steam folder (registry `HKCU\Software\Valve\Steam\SteamPath`);
//! - its libraries (`steamapps\libraryfolders.vdf`) and the one holding CS2;
//! - the CS2 launch options of the Steam user (`userdata\<id>\config\localconfig.vdf`),
//!   to check `-condebug`. Nothing is ever written: the user adds the option.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::vdf;

pub const CS2_APP_ID: &str = "730";
const CS2_FOLDER: &str = "Counter-Strike Global Offensive";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Condebug {
    Present,
    Absent,
    /// Steam's launch options could not be read (no Steam user, manual path).
    Unknown,
}

/// What Crosstalk knows about the CS2 installation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cs2Setup {
    pub steam_dir: Option<String>,
    pub libraries: Vec<String>,
    pub cs2_dir: Option<String>,
    pub console_log: Option<String>,
    pub console_log_exists: bool,
    pub condebug: Condebug,
    pub launch_options: Option<String>,
    /// The path comes from the user (F-04), not from the detection.
    pub manual: bool,
}

/// `console.log` of a CS2 installation folder.
pub fn console_log_of(cs2_dir: &Path) -> PathBuf {
    cs2_dir.join("game").join("csgo").join("console.log")
}

/// Steam folder from the registry (Windows), or `CROSSTALK_STEAM_DIR`.
pub fn steam_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("CROSSTALK_STEAM_DIR") {
        return Some(PathBuf::from(d));
    }
    #[cfg(windows)]
    {
        registry_steam_path()
            .map(|p| PathBuf::from(p.replace('/', "\\")))
            .filter(|p| p.is_dir())
            .or_else(|| {
                let d = PathBuf::from(r"C:\Program Files (x86)\Steam");
                d.is_dir().then_some(d)
            })
    }
    #[cfg(not(windows))]
    None
}

#[cfg(windows)]
fn registry_steam_path() -> Option<String> {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let mut buf = [0u16; 1024];
    let mut len = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Valve\Steam"),
            w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
        .ok()
        .ok()?;
    }
    let n = (len as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]))
}

/// Steam libraries: the Steam folder itself plus those of `libraryfolders.vdf`.
pub fn libraries(steam: &Path) -> Vec<PathBuf> {
    let mut out = vec![steam.to_path_buf()];
    let file = steam.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = std::fs::read_to_string(&file) {
        if let Ok(v) = vdf::parse(&text) {
            if let Some(root) = v.get("libraryfolders") {
                for (_, lib) in root.entries() {
                    if let Some(p) = lib.get("path").and_then(|p| p.as_str()) {
                        let p = PathBuf::from(p);
                        if !out.iter().any(|o| same_path(o, &p)) {
                            out.push(p);
                        }
                    }
                }
            }
        }
    }
    out
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    norm(a) == norm(b)
}

/// The CS2 folder in the first library that has it.
pub fn find_cs2(libraries: &[PathBuf]) -> Option<PathBuf> {
    libraries
        .iter()
        .map(|l| l.join("steamapps").join("common").join(CS2_FOLDER))
        .find(|d| d.join("game").join("csgo").is_dir())
}

/// CS2 launch options of the most recently active Steam user.
pub fn launch_options(steam: &Path) -> Option<String> {
    let users = std::fs::read_dir(steam.join("userdata")).ok()?;
    let mut configs: Vec<(std::time::SystemTime, PathBuf)> = users
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("config").join("localconfig.vdf"))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    configs.sort();
    let (_, latest) = configs.pop()?;
    let v = vdf::parse(&std::fs::read_to_string(latest).ok()?).ok()?;
    let app = v.path(&[
        "UserLocalConfigStore",
        "Software",
        "Valve",
        "Steam",
        "apps",
        CS2_APP_ID,
    ])?;
    Some(
        app.get("LaunchOptions")
            .and_then(|o| o.as_str())
            .unwrap_or("")
            .to_string(),
    )
}

pub fn has_condebug(options: &str) -> bool {
    options
        .split_whitespace()
        .any(|t| t.eq_ignore_ascii_case("-condebug"))
}

/// Manual path (F-04): the CS2 folder, its `game\csgo` folder or `console.log`.
pub fn manual_console_log(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase());
    match name.as_deref() {
        Some("console.log") => path.to_path_buf(),
        Some("csgo") => path.join("console.log"),
        _ => console_log_of(path),
    }
}

/// Full detection, or the user's path when given.
pub fn detect(manual: Option<&Path>) -> Cs2Setup {
    let steam = steam_dir();
    let libs = steam.as_deref().map(libraries).unwrap_or_default();
    let options = steam.as_deref().and_then(launch_options);
    let condebug = match &options {
        Some(o) if has_condebug(o) => Condebug::Present,
        Some(_) => Condebug::Absent,
        None => Condebug::Unknown,
    };
    let (cs2_dir, console_log) = match manual {
        Some(m) => {
            let log = manual_console_log(m);
            let dir = log
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .map(Path::to_path_buf);
            (dir, Some(log))
        }
        None => {
            let dir = find_cs2(&libs);
            let log = dir.as_deref().map(console_log_of);
            (dir, log)
        }
    };
    Cs2Setup {
        steam_dir: steam.map(|p| p.display().to_string()),
        libraries: libs.iter().map(|p| p.display().to_string()).collect(),
        cs2_dir: cs2_dir.map(|p| p.display().to_string()),
        console_log_exists: console_log.as_deref().is_some_and(Path::is_file),
        console_log: console_log.map(|p| p.display().to_string()),
        condebug,
        launch_options: options,
        manual: manual.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_steam(root: &Path, options: &str) -> PathBuf {
        let steam = root.join("Steam");
        let lib = root.join("Games");
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        std::fs::create_dir_all(
            lib.join("steamapps/common")
                .join(CS2_FOLDER)
                .join("game/csgo"),
        )
        .unwrap();
        std::fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n \"0\" {{ \"path\" \"{}\" \"apps\" {{ \"228980\" \"1\" }} }}\n \"1\" {{ \"path\" \"{}\" \"apps\" {{ \"730\" \"2\" }} }}\n}}\n",
                steam.display().to_string().replace('\\', "\\\\"),
                lib.display().to_string().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        for (user, opts, age) in [("111", "-novid", 0u64), ("222", options, 1)] {
            let cfg = steam.join("userdata").join(user).join("config");
            std::fs::create_dir_all(&cfg).unwrap();
            let f = cfg.join("localconfig.vdf");
            std::fs::write(
                &f,
                format!("\"UserLocalConfigStore\" {{ \"Software\" {{ \"Valve\" {{ \"Steam\" {{ \"apps\" {{ \"730\" {{ \"LaunchOptions\" \"{opts}\" }} }} }} }} }} }}"),
            )
            .unwrap();
            // The second user is the most recent one.
            let t = std::time::SystemTime::now() - std::time::Duration::from_secs(100 - age * 50);
            std::fs::File::options()
                .write(true)
                .open(&f)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        steam
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("crosstalk-steam-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn finds_cs2_in_a_second_library_and_reads_launch_options() {
        let root = tmp("detect");
        let steam = fake_steam(&root, "-novid -CONDEBUG +fps_max 0");
        let libs = libraries(&steam);
        assert_eq!(libs.len(), 2);
        let cs2 = find_cs2(&libs).unwrap();
        assert!(cs2.starts_with(root.join("Games")));
        assert!(console_log_of(&cs2).ends_with("game/csgo/console.log"));
        let opts = launch_options(&steam).unwrap();
        assert!(has_condebug(&opts), "{opts}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn condebug_must_be_a_whole_option() {
        assert!(has_condebug("-novid -condebug"));
        assert!(!has_condebug("-condebugx"));
        assert!(!has_condebug(""));
        let root = tmp("absent");
        let steam = fake_steam(&root, "-novid");
        assert!(!has_condebug(&launch_options(&steam).unwrap()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn manual_paths_accept_the_folder_csgo_or_the_log() {
        let d = Path::new("D:/Games/Counter-Strike Global Offensive");
        let want = d.join("game").join("csgo").join("console.log");
        assert_eq!(manual_console_log(d), want);
        assert_eq!(manual_console_log(&d.join("game").join("csgo")), want);
        assert_eq!(manual_console_log(&want), want);
    }
}
