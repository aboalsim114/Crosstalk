//! Follows `console.log` like `tail -f` (F-10): starts at the end of the file,
//! returns the complete lines written since, and starts over when CS2
//! truncates or recreates the file (each game launch). The file is opened for
//! reading only, shared with CS2, and reopened at every poll.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Largest chunk read per poll: a burst is caught up over a few polls.
const MAX_READ: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailEvent {
    /// The file does not exist (yet).
    Missing,
    /// The file shrank or was recreated: reading restarts from its beginning.
    Restarted,
}

pub struct Tailer {
    path: PathBuf,
    pos: u64,
    partial: Vec<u8>,
    created: Option<SystemTime>,
    started: bool,
}

impl Tailer {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            pos: 0,
            partial: Vec::new(),
            created: None,
            started: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bytes already read.
    pub fn offset(&self) -> u64 {
        self.pos
    }

    /// New complete lines; the first poll only positions at the end of the file.
    pub fn poll(&mut self) -> (Vec<String>, Option<TailEvent>) {
        let Ok(meta) = std::fs::metadata(&self.path) else {
            // A file appearing later is read from its start.
            self.started = true;
            self.pos = 0;
            self.partial.clear();
            self.created = None;
            return (Vec::new(), Some(TailEvent::Missing));
        };
        let len = meta.len();
        let created = meta.created().ok();
        if !self.started {
            self.started = true;
            self.pos = len;
            self.created = created;
            return (Vec::new(), None);
        }
        let mut event = None;
        let recreated = created.is_some() && self.created.is_some() && created != self.created;
        if len < self.pos || recreated {
            self.pos = 0;
            self.partial.clear();
            event = Some(TailEvent::Restarted);
        }
        self.created = created;
        if len == self.pos {
            return (Vec::new(), event);
        }
        let mut buf = Vec::new();
        let read = std::fs::File::open(&self.path).and_then(|mut f| {
            f.seek(SeekFrom::Start(self.pos))?;
            f.take(MAX_READ.min(len - self.pos)).read_to_end(&mut buf)
        });
        if read.is_err() {
            return (Vec::new(), event);
        }
        self.pos += buf.len() as u64;
        self.partial.extend_from_slice(&buf);
        let mut lines = Vec::new();
        while let Some(i) = self.partial.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.partial.drain(..=i).collect();
            let text = String::from_utf8_lossy(&line[..line.len() - 1]);
            lines.push(text.trim_end_matches('\r').to_string());
        }
        // A line longer than the cap is not a chat line: drop it.
        if self.partial.len() > 64 * 1024 {
            self.partial.clear();
        }
        (lines, event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("crosstalk-tail-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("console.log")
    }

    fn append(p: &Path, s: &str) {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .unwrap()
            .write_all(s.as_bytes())
            .unwrap();
    }

    #[test]
    fn starts_at_the_end_and_returns_complete_lines() {
        let p = tmp("end");
        append(&p, "old line\n");
        let mut t = Tailer::new(&p);
        assert_eq!(t.poll().0, Vec::<String>::new());
        append(&p, "first\r\nsecond\nhalf");
        assert_eq!(t.poll().0, vec!["first", "second"]);
        append(&p, " done\n");
        assert_eq!(t.poll().0, vec!["half done"]);
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }

    #[test]
    fn restarts_after_truncation_and_waits_for_a_missing_file() {
        let p = tmp("trunc");
        let mut t = Tailer::new(&p);
        assert_eq!(t.poll().1, Some(TailEvent::Missing));
        append(&p, "created later\n");
        assert_eq!(t.poll().0, vec!["created later"]);
        append(&p, "a much longer line to move the offset\n");
        t.poll();
        std::fs::write(&p, "new game\n").unwrap();
        let (lines, ev) = t.poll();
        assert_eq!(ev, Some(TailEvent::Restarted));
        assert_eq!(lines, vec!["new game"]);
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }

    #[test]
    fn invalid_utf8_does_not_stop_the_tail() {
        let p = tmp("utf8");
        std::fs::write(&p, "").unwrap();
        let mut t = Tailer::new(&p);
        t.poll();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&p)
            .unwrap()
            .write_all(b"bad \xff byte\nok\n")
            .unwrap();
        let lines = t.poll().0;
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], "ok");
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }
}
