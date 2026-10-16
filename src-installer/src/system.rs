//! Windows side of the installation: free space, user registration, shortcuts
//! and the autostart entry. Everything is per user (HKCU, the user's Start
//! menu and desktop): no administrator rights are needed. Other systems get
//! inert stand-ins so the installer can be developed and tested anywhere.

use std::path::{Path, PathBuf};

use crate::install::Platform;

/// Registry key of the installation (read by the uninstaller).
pub const APP_KEY: &str = r"Software\Crosstalk";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// Same value name and arguments as the app's own autostart switch
/// (tauri-plugin-autostart: product name, `--minimized`).
const RUN_VALUE: &str = "Crosstalk";
const SHORTCUT: &str = "Crosstalk.lnk";

/// `%LOCALAPPDATA%\Crosstalk`, where the app has always been installed (and
/// where its models live, in `models\`).
pub fn default_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Crosstalk")
}

/// The nearest existing folder (a destination is usually not created yet).
fn existing(dir: &Path) -> Option<&Path> {
    dir.ancestors().find(|p| p.is_dir())
}

pub struct System;

#[cfg(windows)]
mod win {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    pub fn free_space(dir: &Path) -> Option<u64> {
        let dir = existing(dir)?;
        let mut free = 0u64;
        unsafe {
            windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                windows::core::PCWSTR(wide(dir).as_ptr()),
                Some(&mut free),
                None,
                None,
            )
            .ok()?;
        }
        Some(free)
    }

    pub fn volume(dir: &Path) -> Option<String> {
        let dir = existing(dir)?;
        let mut buf = [0u16; 261];
        unsafe {
            windows::Win32::Storage::FileSystem::GetVolumePathNameW(
                windows::core::PCWSTR(wide(dir).as_ptr()),
                &mut buf,
            )
            .ok()?;
        }
        let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..n]).to_lowercase())
    }

    pub fn known_folder(id: &windows::core::GUID) -> Result<PathBuf, String> {
        use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};
        unsafe {
            let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).map_err(|e| e.to_string())?;
            let s = p.to_string().map_err(|e| e.to_string());
            windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _));
            s.map(PathBuf::from)
        }
    }

    pub fn hkcu() -> RegKey {
        RegKey::predef(HKEY_CURRENT_USER)
    }

    pub fn register(dir: &Path, version: &str) -> Result<(), String> {
        let (key, _) = hkcu().create_subkey(APP_KEY).map_err(|e| e.to_string())?;
        key.set_value("InstallDir", &dir.as_os_str())
            .map_err(|e| e.to_string())?;
        key.set_value("Version", &version)
            .map_err(|e| e.to_string())
    }

    pub fn installed_version() -> Option<String> {
        hkcu().open_subkey(APP_KEY).ok()?.get_value("Version").ok()
    }

    pub fn installed_dir() -> Option<PathBuf> {
        let d: String = hkcu()
            .open_subkey(APP_KEY)
            .ok()?
            .get_value("InstallDir")
            .ok()?;
        Some(PathBuf::from(d))
    }

    pub fn desktop_link_exists() -> bool {
        use windows::Win32::UI::Shell::FOLDERID_Desktop;
        known_folder(&FOLDERID_Desktop).is_ok_and(|d| d.join(SHORTCUT).exists())
    }

    pub fn link(exe: &Path, at: &Path) -> Result<(), String> {
        let mut sl = mslnk::ShellLink::new(exe).map_err(|e| e.to_string())?;
        sl.set_working_dir(exe.parent().map(|d| d.display().to_string()));
        sl.set_icon_location(Some(exe.display().to_string()));
        sl.set_name(Some(
            "Traduction en temps réel pour Counter-Strike 2".into(),
        ));
        sl.create_lnk(at).map_err(|e| e.to_string())
    }

    pub fn set_autostart(exe: &Path, on: bool) -> Result<(), String> {
        let (run, _) = hkcu().create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
        if on {
            run.set_value(RUN_VALUE, &format!("\"{}\" --minimized", exe.display()))
                .map_err(|e| e.to_string())?;
            // Re-enable it if the user once turned it off in the Task Manager.
            if let Ok(k) = hkcu().open_subkey_with_flags(RUN_APPROVED_KEY, KEY_SET_VALUE) {
                let enabled = winreg::RegValue {
                    vtype: winreg::enums::RegType::REG_BINARY,
                    bytes: vec![2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into(),
                };
                let _ = k.set_raw_value(RUN_VALUE, &enabled);
            }
            Ok(())
        } else {
            match run.delete_value(RUN_VALUE) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        }
    }
}

impl Platform for System {
    fn free_space(&self, dir: &Path) -> Option<u64> {
        #[cfg(windows)]
        return win::free_space(dir);
        #[cfg(not(windows))]
        {
            let _ = existing(dir);
            None
        }
    }

    fn same_volume(&self, a: &Path, b: &Path) -> bool {
        #[cfg(windows)]
        return matches!((win::volume(a), win::volume(b)), (Some(x), Some(y)) if x == y);
        #[cfg(not(windows))]
        {
            let _ = (a, b);
            true
        }
    }

    fn register(&self, dir: &Path, version: &str) -> Result<(), String> {
        #[cfg(windows)]
        return win::register(dir, version)
            .map_err(|e| format!("Enregistrement dans Windows impossible : {e}"));
        #[cfg(not(windows))]
        {
            let _ = (dir, version);
            Ok(())
        }
    }

    fn shortcuts(&self, exe: &Path, desktop: bool) -> Result<(), String> {
        #[cfg(windows)]
        {
            use windows::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Programs};
            let fail = |e: String| format!("Création du raccourci impossible : {e}");
            let programs = win::known_folder(&FOLDERID_Programs).map_err(fail)?;
            win::link(exe, &programs.join(SHORTCUT)).map_err(fail)?;
            let desk = win::known_folder(&FOLDERID_Desktop)
                .map_err(fail)?
                .join(SHORTCUT);
            if desktop {
                win::link(exe, &desk).map_err(fail)?;
            } else {
                let _ = std::fs::remove_file(desk);
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = (exe, desktop, SHORTCUT);
            Ok(())
        }
    }
}

/// Version recorded by a previous installation, if any.
pub fn installed_version() -> Option<String> {
    #[cfg(windows)]
    return win::installed_version();
    #[cfg(not(windows))]
    None
}

/// Folder of the current installation (update in place).
pub fn installed_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    return win::installed_dir();
    #[cfg(not(windows))]
    None
}

/// Whether the user kept the desktop shortcut (an update keeps their choice).
pub fn desktop_link_exists() -> bool {
    #[cfg(windows)]
    return win::desktop_link_exists();
    #[cfg(not(windows))]
    true
}

/// Launch with Windows (the app's own switch writes the same entry).
pub fn set_autostart(exe: &Path, on: bool) -> Result<(), String> {
    #[cfg(windows)]
    return win::set_autostart(exe, on);
    #[cfg(not(windows))]
    {
        let _ = (exe, on, APP_KEY, RUN_KEY, RUN_APPROVED_KEY, RUN_VALUE);
        Ok(())
    }
}
