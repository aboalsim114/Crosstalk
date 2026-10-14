//! Local history (cahier des charges §4.6): off by default, text only (never
//! audio), on this PC only. One JSON line per final subtitle in
//! `%LOCALAPPDATA%\Crosstalk\history.jsonl`; lines older than 7 days are
//! purged, and the file is capped.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const KEEP_DAYS: u64 = 7;
const MAX_ENTRIES: usize = 20_000;
const DAY_MS: u64 = 24 * 3600 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Unix time, milliseconds.
    pub ts: u64,
    /// `voice` or `chat`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    pub lang: String,
    pub original: String,
    /// French text, when translated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translated: Option<String>,
    /// `final`, `original`, `erreur` or `transcrit` (left untranslated).
    pub status: String,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn default_path() -> PathBuf {
    if let Some(d) = std::env::var_os("CROSSTALK_HISTORY_DIR") {
        return PathBuf::from(d).join("history.jsonl");
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Crosstalk")
        .join("history.jsonl")
}

pub struct History {
    path: PathBuf,
    lock: Mutex<()>,
}

impl History {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    fn read(&self) -> Vec<Entry> {
        std::fs::read_to_string(&self.path)
            .map(|t| {
                t.lines()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn rewrite(&self, entries: &[Entry]) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = self.path.with_extension("jsonl.tmp");
        let mut text = String::new();
        for e in entries {
            text.push_str(&serde_json::to_string(e).map_err(|e| e.to_string())?);
            text.push('\n');
        }
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    /// Drops entries older than 7 days and keeps the newest `MAX_ENTRIES`.
    pub fn purge(&self, now: u64) -> Result<usize, String> {
        let _g = self.lock.lock().unwrap();
        let all = self.read();
        let before = all.len();
        let cutoff = now.saturating_sub(KEEP_DAYS * DAY_MS);
        let mut kept: Vec<Entry> = all.into_iter().filter(|e| e.ts >= cutoff).collect();
        if kept.len() > MAX_ENTRIES {
            kept.drain(..kept.len() - MAX_ENTRIES);
        }
        if kept.len() != before {
            self.rewrite(&kept)?;
        }
        Ok(before - kept.len())
    }

    pub fn append(&self, entry: &Entry) -> Result<(), String> {
        if entry.original.trim().is_empty() || entry.original.len() > 2000 {
            return Err("ligne invalide".into());
        }
        let _g = self.lock.lock().unwrap();
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| e.to_string())?;
        writeln!(
            f,
            "{}",
            serde_json::to_string(entry).map_err(|e| e.to_string())?
        )
        .map_err(|e| e.to_string())
    }

    /// Newest first; `query` matches original, French or sender, ignoring case.
    pub fn list(&self, query: &str, limit: usize) -> Vec<Entry> {
        let _g = self.lock.lock().unwrap();
        let q = query.trim().to_lowercase();
        let mut all = self.read();
        all.reverse();
        all.into_iter()
            .filter(|e| {
                q.is_empty()
                    || e.original.to_lowercase().contains(&q)
                    || e.translated
                        .as_deref()
                        .is_some_and(|t| t.to_lowercase().contains(&q))
                    || e.sender
                        .as_deref()
                        .is_some_and(|s| s.to_lowercase().contains(&q))
            })
            .take(limit)
            .collect()
    }

    pub fn count(&self) -> usize {
        let _g = self.lock.lock().unwrap();
        self.read().len()
    }

    pub fn clear(&self) -> Result<(), String> {
        let _g = self.lock.lock().unwrap();
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

/// Plain-text export, oldest first.
pub fn to_text(entries: &[Entry]) -> String {
    let mut out = String::from("Historique Crosstalk (texte uniquement)\n\n");
    for e in entries.iter().rev() {
        let secs = (e.ts / 1000) as i64;
        let who = match (&e.channel, &e.sender) {
            (Some(c), Some(s)) => format!("CHAT {c} · {s}"),
            (Some(c), None) => format!("CHAT {c}"),
            _ => "VOIX".to_string(),
        };
        out.push_str(&format!(
            "[{}] {who} · {}\n",
            utc(secs),
            e.lang.to_uppercase()
        ));
        out.push_str(&format!("  {}\n", e.original));
        if let Some(t) = &e.translated {
            if t != &e.original {
                out.push_str(&format!("  → {t}\n"));
            }
        }
    }
    out
}

/// `YYYY-MM-DD HH:MM:SS UTC` without a date library.
pub(crate) fn utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// `Documents\Crosstalk-historique-<date>.<ext>`.
pub fn export_path(ext: &str, now: u64) -> PathBuf {
    documents_path("Crosstalk-historique", ext, now)
}

/// `Documents\<prefix>-<date>.<ext>` (temp folder if Documents is missing).
pub(crate) fn documents_path(prefix: &str, ext: &str, now: u64) -> PathBuf {
    let base = std::env::var_os("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Documents"))
        .filter(|p| p.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    let stamp = utc((now / 1000) as i64)
        .replace([':', ' '], "-")
        .replace("-UTC", "");
    base.join(format!("{prefix}-{stamp}.{ext}"))
}

pub fn export(h: &History, format: &str, to: &Path) -> Result<usize, String> {
    let entries = h.list("", usize::MAX);
    let text = match format {
        "json" => serde_json::to_string_pretty(&entries.iter().rev().collect::<Vec<_>>())
            .map_err(|e| e.to_string())?,
        "txt" => to_text(&entries),
        other => return Err(format!("format inconnu : {other}")),
    };
    std::fs::write(to, text).map_err(|e| format!("{} : {e}", to.display()))?;
    Ok(entries.len())
}

// ---------- Tauri ----------

#[tauri::command]
pub fn history_append(h: tauri::State<'_, History>, mut entry: Entry) -> Result<(), String> {
    if entry.ts == 0 {
        entry.ts = now_ms();
    }
    h.append(&entry)
}

#[tauri::command]
pub fn history_list(h: tauri::State<'_, History>, query: String, limit: usize) -> Vec<Entry> {
    // Crosstalk can stay open in the tray for days: purge here too.
    let _ = h.purge(now_ms());
    h.list(&query, limit.min(2000))
}

#[tauri::command]
pub fn history_count(h: tauri::State<'_, History>) -> usize {
    h.count()
}

#[tauri::command]
pub fn history_clear(h: tauri::State<'_, History>) -> Result<(), String> {
    h.clear()
}

/// Writes the export to Documents and returns its path.
#[tauri::command]
pub fn history_export(h: tauri::State<'_, History>, format: String) -> Result<String, String> {
    let path = export_path(&format, now_ms());
    export(&h, &format, &path)?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(ts: u64, original: &str, translated: Option<&str>) -> Entry {
        Entry {
            ts,
            kind: "voice".into(),
            channel: None,
            sender: None,
            lang: "ru".into(),
            original: original.into(),
            translated: translated.map(Into::into),
            status: "final".into(),
        }
    }

    fn tmp(name: &str) -> History {
        let d =
            std::env::temp_dir().join(format!("crosstalk-history-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        History::new(d.join("history.jsonl"))
    }

    #[test]
    fn appends_lists_newest_first_and_searches() {
        let h = tmp("list");
        assert!(h.list("", 10).is_empty());
        h.append(&entry(1, "Двое на B", Some("deux sur B")))
            .unwrap();
        h.append(&Entry {
            kind: "chat".into(),
            channel: Some("CT".into()),
            sender: Some("Bob".into()),
            ..entry(2, "rush b", Some("rush B"))
        })
        .unwrap();
        let all = h.list("", 10);
        assert_eq!(all.iter().map(|e| e.ts).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!(h.list("DEUX", 10).len(), 1);
        assert_eq!(h.list("bob", 10)[0].ts, 2);
        assert_eq!(h.list("", 1).len(), 1);
        assert!(h.append(&entry(3, "  ", None)).is_err());
        h.clear().unwrap();
        assert_eq!(h.count(), 0);
        h.clear().unwrap();
    }

    #[test]
    fn purges_after_seven_days() {
        let h = tmp("purge");
        let now = 100 * DAY_MS;
        h.append(&entry(now - 8 * DAY_MS, "old", None)).unwrap();
        h.append(&entry(now - DAY_MS, "recent", None)).unwrap();
        assert_eq!(h.purge(now).unwrap(), 1);
        assert_eq!(h.list("", 10)[0].original, "recent");
        assert_eq!(h.purge(now).unwrap(), 0);
    }

    #[test]
    fn exports_text_and_json() {
        let h = tmp("export");
        h.append(&entry(1_760_313_600_000, "Двое на B", Some("deux sur B")))
            .unwrap();
        h.append(&Entry {
            kind: "chat".into(),
            channel: Some("ALL".into()),
            sender: Some("Ann".into()),
            ..entry(1_760_313_660_000, "gg", None)
        })
        .unwrap();
        let txt = to_text(&h.list("", 10));
        assert!(
            txt.contains("[2025-10-13 00:00:00 UTC] VOIX · RU\n  Двое на B\n  → deux sur B"),
            "{txt}"
        );
        assert!(
            txt.find("Двое").unwrap() < txt.find("gg").unwrap(),
            "oldest first"
        );
        assert!(txt.contains("CHAT ALL · Ann"));
        let dir = std::env::temp_dir().join(format!("crosstalk-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("h.json");
        assert_eq!(export(&h, "json", &p).unwrap(), 2);
        let back: Vec<Entry> = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(back[0].original, "Двое на B");
        assert!(export(&h, "pdf", &p).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn utc_dates() {
        assert_eq!(utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(utc(1_791_849_599), "2026-10-12 23:59:59 UTC");
    }
}
