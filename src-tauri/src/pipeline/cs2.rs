//! Counter-Strike 2 integration, strictly external: the game is launched with
//! `-condebug` and Crosstalk tails the `console.log` it writes. No injection, no
//! memory reads, no network interception.

use std::path::{Path, PathBuf};

use super::Utterance;

/// Location of `console.log` inside a Steam library folder.
pub fn console_log_path(steam_library: &Path) -> PathBuf {
    steam_library
        .join("steamapps")
        .join("common")
        .join("Counter-Strike Global Offensive")
        .join("game")
        .join("csgo")
        .join("console.log")
}

pub trait ChatSource: Send {
    /// New chat lines since the last call.
    fn poll(&mut self) -> Vec<Utterance>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_console_log_path() {
        let p = console_log_path(Path::new("D:/SteamLibrary"));
        assert!(p.ends_with("Counter-Strike Global Offensive/game/csgo/console.log"));
    }
}
