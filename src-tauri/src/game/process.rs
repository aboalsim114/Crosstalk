//! Finding `cs2.exe` among running processes.
//!
//! Uses a Toolhelp snapshot, which only lists executable names and PIDs. The
//! game process is never opened (no `OpenProcess`, no memory access): see the
//! non-negotiable constraints in docs/cahier-des-charges.md §3.

/// Executable name of Counter-Strike 2.
pub const CS2_EXE: &str = "cs2.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
}

/// Picks the PID of `exe` (case-insensitive). With several matches the lowest
/// PID wins, so the choice is stable between polls.
pub fn find_pid(entries: &[ProcessEntry], exe: &str) -> Option<u32> {
    entries
        .iter()
        .filter(|e| e.name.eq_ignore_ascii_case(exe))
        .map(|e| e.pid)
        .min()
}

/// Lists running processes (name and PID only).
#[cfg(windows)]
pub fn list_processes() -> std::io::Result<Vec<ProcessEntry>> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    // SAFETY: plain Win32 calls on a snapshot handle we own and close.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| std::io::Error::other(e.message()))?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut out = Vec::new();
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            out.push(ProcessEntry {
                pid: entry.th32ProcessID,
                name: String::from_utf16_lossy(&entry.szExeFile[..len]),
            });
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
        Ok(out)
    }
}

#[cfg(not(windows))]
pub fn list_processes() -> std::io::Result<Vec<ProcessEntry>> {
    Ok(Vec::new())
}

/// Name of a process by PID, from the same snapshot (used for display only).
pub fn process_name(pid: u32) -> Option<String> {
    list_processes()
        .ok()?
        .into_iter()
        .find(|e| e.pid == pid)
        .map(|e| e.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(pid: u32, name: &str) -> ProcessEntry {
        ProcessEntry {
            pid,
            name: name.into(),
        }
    }

    #[test]
    fn finds_cs2_case_insensitively() {
        let list = [
            entry(4, "System"),
            entry(900, "CS2.EXE"),
            entry(12, "steam.exe"),
        ];
        assert_eq!(find_pid(&list, CS2_EXE), Some(900));
    }

    #[test]
    fn picks_the_lowest_pid_when_several_match() {
        let list = [entry(900, "cs2.exe"), entry(300, "cs2.exe")];
        assert_eq!(find_pid(&list, CS2_EXE), Some(300));
    }

    #[test]
    fn ignores_similar_names() {
        let list = [
            entry(1, "cs2.exe.bak"),
            entry(2, "xcs2.exe"),
            entry(3, "csgo.exe"),
        ];
        assert_eq!(find_pid(&list, CS2_EXE), None);
    }

    #[cfg(windows)]
    #[test]
    fn snapshot_lists_the_current_process() {
        let me = std::process::id();
        let list = list_processes().expect("toolhelp snapshot");
        assert!(
            list.iter().any(|e| e.pid == me),
            "current process not listed"
        );
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_name().unwrap().to_string_lossy();
        assert!(process_name(me).unwrap().eq_ignore_ascii_case(&name));
    }
}
