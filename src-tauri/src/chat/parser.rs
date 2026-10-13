//! Turns `console.log` lines into chat messages (F-11, F-12).
//!
//! The patterns live in a rules file (`rules.json`, embedded), which a copy in
//! `%APPDATA%\Crosstalk\chat-rules.json` overrides: a change of the game's
//! format is fixed by editing that file, without a new version. Lines that
//! match no rule (everything that is not chat) are ignored, as are empty
//! messages and the same message repeated within 2 s.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Serialize};

pub const DEFAULT_RULES: &str = include_str!("rules.json");
const DEDUP_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    #[serde(rename = "ALL")]
    All,
    #[serde(rename = "CT")]
    Ct,
    #[serde(rename = "T")]
    T,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatLine {
    pub channel: Channel,
    pub sender: String,
    pub message: String,
    pub location: Option<String>,
    pub dead: bool,
}

#[derive(Deserialize)]
struct RulesFile {
    version: String,
    rules: Vec<RuleSpec>,
    channels: std::collections::HashMap<String, Channel>,
}

#[derive(Deserialize)]
struct RuleSpec {
    name: String,
    pattern: String,
}

pub struct Rules {
    pub version: String,
    /// Where the rules come from: `None` for the embedded ones.
    pub source: Option<PathBuf>,
    rules: Vec<(String, Regex)>,
    channels: std::collections::HashMap<String, Channel>,
}

impl Rules {
    pub fn parse(json: &str, source: Option<PathBuf>) -> Result<Self, String> {
        let f: RulesFile =
            serde_json::from_str(json).map_err(|e| format!("règles de chat : {e}"))?;
        let mut rules = Vec::new();
        for r in f.rules {
            let re = Regex::new(&r.pattern).map_err(|e| format!("règle « {} » : {e}", r.name))?;
            for g in ["channel", "sender", "message"] {
                if !re.capture_names().flatten().any(|n| n == g) {
                    return Err(format!("règle « {} » : groupe « {g} » manquant", r.name));
                }
            }
            rules.push((r.name, re));
        }
        if rules.is_empty() {
            return Err("règles de chat : aucune règle".into());
        }
        Ok(Self {
            version: f.version,
            source,
            rules,
            channels: f.channels,
        })
    }

    pub fn embedded() -> Self {
        Self::parse(DEFAULT_RULES, None).expect("embedded chat rules")
    }

    /// The user's rules file when present and valid; otherwise the embedded
    /// rules, with the reason the file was not used.
    pub fn load(user_file: Option<&Path>) -> (Self, Option<String>) {
        let Some(path) = user_file.filter(|p| p.is_file()) else {
            return (Self::embedded(), None);
        };
        match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| Self::parse(&t, Some(path.into())))
        {
            Ok(r) => (r, None),
            Err(e) => (
                Self::embedded(),
                Some(format!("{} ignoré : {e}", path.display())),
            ),
        }
    }

    pub fn parse_line(&self, line: &str) -> Option<ChatLine> {
        for (_, re) in &self.rules {
            let Some(c) = re.captures(line) else { continue };
            let channel = *self.channels.get(c.name("channel")?.as_str())?;
            let clean = |s: &str| s.replace(['\u{200e}', '\u{200f}'], "").trim().to_string();
            let sender = clean(c.name("sender")?.as_str());
            let message = clean(c.name("message")?.as_str());
            if sender.is_empty() || message.is_empty() {
                return None;
            }
            return Some(ChatLine {
                channel,
                sender,
                message,
                location: c
                    .name("location")
                    .map(|l| clean(l.as_str()))
                    .filter(|l| !l.is_empty()),
                dead: c.name("dead").is_some() || c.name("dead1").is_some(),
            });
        }
        None
    }
}

/// The shape of a line that looks like chat (`[TAG] … : …`) but matched no
/// rule, for the journal: the tag is kept (it is the game's label), letters
/// become `x` and digits `9`, so no name or message is ever written.
pub fn chat_shape(line: &str) -> Option<String> {
    let line = line.trim();
    let rest =
        line.trim_start_matches(|c: char| c.is_ascii_digit() || matches!(c, '/' | ':' | '.' | ' '));
    let rest = rest.strip_prefix('[')?;
    let (tag, tail) = rest.split_once(']')?;
    let tag_ok = !tag.is_empty()
        && tag.chars().count() <= 16
        && tag
            .chars()
            .all(|c| c.is_uppercase() || matches!(c, '(' | ')' | '-' | ' '));
    if !tag_ok || !tail.contains(':') || tail.trim().is_empty() {
        return None;
    }
    let mut shape = format!("[{tag}]");
    let mut last_x = false;
    for c in tail.chars().take(120) {
        if matches!(c, '\u{200e}' | '\u{200f}') {
            continue;
        }
        if c.is_alphabetic() {
            if !last_x {
                shape.push('x');
            }
            last_x = true;
            continue;
        }
        last_x = false;
        shape.push(match c {
            '0'..='9' => '9',
            c if c.is_control() => '?',
            c => c,
        });
    }
    Some(shape)
}

/// Drops a message repeated by the same player within 2 s (F-12).
#[derive(Default)]
pub struct Dedup {
    recent: VecDeque<(Instant, Channel, String, String)>,
}

impl Dedup {
    pub fn fresh(&mut self, line: &ChatLine, now: Instant) -> bool {
        while self
            .recent
            .front()
            .is_some_and(|(t, ..)| now.duration_since(*t) > DEDUP_WINDOW)
        {
            self.recent.pop_front();
        }
        let dup = self
            .recent
            .iter()
            .any(|(_, c, s, m)| *c == line.channel && *s == line.sender && *m == line.message);
        if !dup {
            self.recent
                .push_back((now, line.channel, line.sender.clone(), line.message.clone()));
        }
        !dup
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(line: &str) -> Option<ChatLine> {
        Rules::embedded().parse_line(line)
    }

    #[test]
    fn reads_cs2_chat_lines() {
        let l = p("10/12 21:03:11  [ALL] Steve\u{200e}: gg wp").unwrap();
        assert_eq!(
            (l.channel, l.sender.as_str(), l.message.as_str(), l.dead),
            (Channel::All, "Steve", "gg wp", false)
        );
        let l = p("[CT] Иван Petrov\u{200e}\u{fe6b}Bombsite A: Двое на B").unwrap();
        assert_eq!(l.channel, Channel::Ct);
        assert_eq!(l.sender, "Иван Petrov");
        assert_eq!(l.location.as_deref(), Some("Bombsite A"));
        assert_eq!(l.message, "Двое на B");
        let l = p("[T] xX_sniper_Xx\u{200e} [DEAD]: rotate b: they are mid").unwrap();
        assert!(l.dead);
        assert_eq!(l.sender, "xX_sniper_Xx");
        assert_eq!(l.message, "rotate b: they are mid");
        let l = p("10/12/2026 21:03:11.512 [T] a: b").unwrap();
        assert_eq!((l.sender.as_str(), l.message.as_str()), ("a", "b"));
    }

    #[test]
    fn reads_the_french_and_russian_game_formats() {
        // French CS2: [GÉNÉRAL] / [AT], "[MORT(E)]", "@Lieu", space before the colon.
        let l = p("[AT] Боец_7 @Départ AT : идём на Б").unwrap();
        assert_eq!(
            (
                l.channel,
                l.sender.as_str(),
                l.location.as_deref(),
                l.message.as_str(),
                l.dead
            ),
            (Channel::Ct, "Боец_7", Some("Départ AT"), "идём на Б", false)
        );
        let l = p("[AT] Боец_7 [MORT(E)] : один лоу").unwrap();
        assert_eq!(
            (l.sender.as_str(), l.message.as_str(), l.dead),
            ("Боец_7", "один лоу", true)
        );
        let l = p("[GÉNÉRAL] Игрок2 : гг").unwrap();
        assert_eq!(
            (l.channel, l.sender.as_str(), l.message.as_str()),
            (Channel::All, "Игрок2", "гг")
        );
        let l = p("[AT] • Player One @Départ AT : rotate A").unwrap();
        assert_eq!(
            (l.sender.as_str(), l.message.as_str()),
            ("Player One", "rotate A")
        );
        let l = p("[T] *MORT* Bob : eco").unwrap();
        assert!(l.dead && l.channel == Channel::T && l.sender == "Bob");
        let l = p("[ВСЕ] Саша [МЁРТВ]: гг").unwrap();
        assert_eq!(
            (l.channel, l.sender.as_str(), l.dead),
            (Channel::All, "Саша", true)
        );
        let l = p("[КТ] Саша: раш б").unwrap();
        assert_eq!(l.channel, Channel::Ct);
    }

    #[test]
    fn ignores_everything_that_is_not_chat() {
        for line in [
            "Damage Given to \"Bot Gary\" - 27 in 1 hit",
            "ChangeGameUIState: CSGO_GAME_UI_STATE_LOADINGSCREEN -> CSGO_GAME_UI_STATE_INGAME",
            "[Client] CL:  CCSGO_BlurTarget",
            "[ALL] Steve\u{200e}: ",
            "[ALL] Steve",
            "[SPEC] Steve: hi",
            "",
        ] {
            assert_eq!(p(line), None, "{line:?}");
        }
    }

    #[test]
    fn user_rules_override_and_broken_ones_fall_back() {
        let dir = std::env::temp_dir().join(format!("crosstalk-rules-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("chat-rules.json");
        std::fs::write(&f, r#"{"version":"test-1","rules":[{"name":"x","pattern":"^<(?P<channel>A)> (?P<sender>\\w+) says (?P<message>.+)$"}],"channels":{"A":"ALL"}}"#).unwrap();
        let (r, warn) = Rules::load(Some(&f));
        assert!(warn.is_none());
        assert_eq!(r.version, "test-1");
        assert_eq!(r.parse_line("<A> bob says hi").unwrap().message, "hi");
        std::fs::write(
            &f,
            r#"{"version":"bad","rules":[{"name":"x","pattern":"^(?P<sender>x)$"}],"channels":{}}"#,
        )
        .unwrap();
        let (r, warn) = Rules::load(Some(&f));
        assert_eq!(r.version, Rules::embedded().version);
        assert!(warn.unwrap().contains("channel"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_chat_lines_are_logged_by_shape_only() {
        assert_eq!(
            chat_shape("[ÉQUIPE] Jean Dupont @Milieu : on rush 2 fois").as_deref(),
            Some("[ÉQUIPE] x x @x : x x 9 x")
        );
        assert_eq!(chat_shape("Damage Given to \"Bot\" - 27 in 1 hit"), None);
        assert_eq!(chat_shape("[Client] CL: x"), None);
        assert_eq!(chat_shape("+2900 $ à l'équipe pour avoir perdu."), None);
    }

    #[test]
    fn repeats_within_two_seconds_are_dropped() {
        let mut d = Dedup::default();
        let l = p("[ALL] a: gg").unwrap();
        let t = Instant::now();
        assert!(d.fresh(&l, t));
        assert!(!d.fresh(&l, t + Duration::from_millis(1500)));
        assert!(d.fresh(&p("[ALL] b: gg").unwrap(), t + Duration::from_millis(1600)));
        assert!(d.fresh(&l, t + Duration::from_millis(2600)));
    }
}
