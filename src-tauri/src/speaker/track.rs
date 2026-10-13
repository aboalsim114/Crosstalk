//! Who was talking: names read on CS2's voice indicator, matched to the time
//! of a voice line. Pure logic, tested on Linux; screen capture and OCR live
//! in `screen.rs` and `ocr.rs`.
//!
//! OCR is noisy ("Sasha" one frame, "Sasba" the next): readings are grouped
//! under a known name when they are close enough, and a new name only counts
//! once it was read twice. A line gets a name only when one name clearly
//! dominates while it was spoken: no guess.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Readings older than this are forgotten.
const KEEP: Duration = Duration::from_secs(30);
/// Margin around a voice line (the indicator appears / fades a bit apart
/// from the audio).
const BEFORE: Duration = Duration::from_millis(400);
const AFTER: Duration = Duration::from_millis(600);
/// Share of the readings a name needs to be credited with a line.
const DOMINANT: f32 = 0.6;
/// Readings of a new name before it is trusted.
const CONFIRM: u32 = 2;
/// Known names kept per session (a match has 10 players).
const MAX_KNOWN: usize = 24;

/// One OCR reading of the indicator area.
#[derive(Debug, Clone)]
struct Sighting {
    at: Instant,
    /// Indexes into `Tracker::known`.
    names: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Known {
    name: String,
    key: String,
    seen: u32,
}

#[derive(Debug, Default)]
pub struct Tracker {
    known: Vec<Known>,
    sightings: VecDeque<Sighting>,
}

/// A name as CS2 shows it, from an OCR line; `None` for HUD noise.
pub fn clean_name(raw: &str) -> Option<String> {
    // The speaker icon is often read as a stray symbol before the name.
    let s = raw
        .trim()
        .trim_start_matches(|c: char| !c.is_alphanumeric() && !"[(<".contains(c))
        .trim_end_matches(|c: char| c.is_whitespace() || "|·•".contains(c))
        .trim();
    let letters = s.chars().filter(|c| c.is_alphabetic()).count();
    let len = s.chars().count();
    if !(2..=32).contains(&len) || letters == 0 {
        return None;
    }
    // Chat lines ("[ALL] Name: message") and money / timers are not names.
    if s.starts_with('[') || s.contains(": ") || s.ends_with(':') || s.contains('$') {
        return None;
    }
    if s.chars().filter(|c| c.is_ascii_digit()).count() * 2 > len {
        return None;
    }
    Some(s.to_string())
}

/// Comparison key: case, spaces and look-alike letters folded.
fn key(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'l' | '|' => 'i',
            // Cyrillic letters OCR mixes up with Latin ones.
            'а' => 'a',
            'е' => 'e',
            'о' => 'o',
            'р' => 'p',
            'с' => 'c',
            'х' => 'x',
            'у' => 'y',
            c => c,
        })
        .collect()
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != *cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Two readings of the same name (up to one wrong letter in five). Digits
/// must match: "Player1" and "Player2" are two teammates.
fn same(a: &str, b: &str) -> bool {
    let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
    if digits(a) != digits(b) {
        return false;
    }
    let longest = a.chars().count().max(b.chars().count());
    distance(a, b) * 5 <= longest
}

impl Tracker {
    /// Names currently trusted, most seen first.
    pub fn known(&self) -> Vec<String> {
        let mut k: Vec<&Known> = self.known.iter().filter(|k| k.seen >= CONFIRM).collect();
        k.sort_by_key(|k| std::cmp::Reverse(k.seen));
        k.into_iter().map(|k| k.name.clone()).collect()
    }

    pub fn clear(&mut self) {
        self.known.clear();
        self.sightings.clear();
    }

    /// A name known from elsewhere (a chat sender): trusted from its first
    /// OCR reading.
    pub fn confirm(&mut self, name: &str) {
        let Some(name) = clean_name(name) else {
            return;
        };
        let i = self.intern(name);
        let n = &mut self.known[i];
        n.seen = n.seen.max(CONFIRM);
    }

    fn intern(&mut self, name: String) -> usize {
        let k = key(&name);
        if let Some(i) = self.known.iter().position(|n| same(&n.key, &k)) {
            let n = &mut self.known[i];
            n.seen = n.seen.saturating_add(1);
            // Keep the spelling read most often: the longest is usually right
            // when a letter was dropped.
            if n.seen <= CONFIRM && name.chars().count() > n.name.chars().count() {
                n.name = name;
                n.key = k;
            }
            return i;
        }
        if self.known.len() >= MAX_KNOWN {
            // Forget the least seen name.
            if let Some((i, _)) = self.known.iter().enumerate().min_by_key(|(_, n)| n.seen) {
                self.known.remove(i);
                for s in &mut self.sightings {
                    s.names.retain(|&x| x != i);
                    s.names.iter_mut().filter(|x| **x > i).for_each(|x| *x -= 1);
                }
            }
        }
        self.known.push(Known {
            name,
            key: k,
            seen: 1,
        });
        self.known.len() - 1
    }

    /// One OCR reading: the lines read in the indicator area at `at`.
    /// Returns the names read (cleaned).
    pub fn observe(&mut self, at: Instant, lines: &[String]) -> Vec<String> {
        let names: Vec<String> = lines.iter().filter_map(|l| clean_name(l)).collect();
        let mut ids: Vec<usize> = names.iter().map(|n| self.intern(n.clone())).collect();
        ids.dedup();
        self.sightings.push_back(Sighting { at, names: ids });
        while self
            .sightings
            .front()
            .is_some_and(|s| at.saturating_duration_since(s.at) > KEEP)
        {
            self.sightings.pop_front();
        }
        names
    }

    /// The name shown while a voice line from `start` to `end` was spoken, if
    /// one clearly dominates.
    pub fn speaker_between(&self, start: Instant, end: Instant) -> Option<String> {
        let from = start.checked_sub(BEFORE).unwrap_or(start);
        let to = end + AFTER;
        let mut counts = vec![0u32; self.known.len()];
        let mut with_names = 0u32;
        for s in self.sightings.iter().filter(|s| s.at >= from && s.at <= to) {
            if s.names.is_empty() {
                continue;
            }
            with_names += 1;
            for &i in &s.names {
                counts[i] += 1;
            }
        }
        let (best, &n) = counts.iter().enumerate().max_by_key(|(_, c)| **c)?;
        let trusted = self.known[best].seen >= CONFIRM;
        // Two names shown together the whole time: no way to tell who spoke.
        let tie = counts.iter().enumerate().any(|(i, &c)| i != best && c == n);
        (n > 0 && trusted && !tie && n as f32 >= with_names as f32 * DOMINANT)
            .then(|| self.known[best].name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn hud_noise_is_not_a_name() {
        assert_eq!(clean_name("  )) Боец_7 "), Some("Боец_7".into()));
        assert_eq!(clean_name("• Player One |"), Some("Player One".into()));
        assert_eq!(clean_name("[ALL] Player One: rush b"), None);
        assert_eq!(clean_name("$4750"), None);
        assert_eq!(clean_name("1:45"), None);
        assert_eq!(clean_name("x"), None);
        assert_eq!(clean_name("12345a"), None);
        assert_eq!(clean_name("sniper2000"), Some("sniper2000".into()));
    }

    #[test]
    fn ocr_misreadings_are_grouped_under_one_name() {
        let mut t = Tracker::default();
        let now = Instant::now();
        t.observe(now, &lines(&["Sasha"]));
        t.observe(now, &lines(&["Sasba"]));
        t.observe(now, &lines(&["SASHA"]));
        t.observe(now, &lines(&["Игрок2"]));
        assert_eq!(
            t.known(),
            vec!["Sasha".to_string()],
            "one reading is not trusted yet"
        );
        t.observe(now, &lines(&["Игpок2"])); // Latin p
        assert_eq!(t.known().len(), 2);
        assert!(!same(&key("Max"), &key("Mia")));
        assert!(!same(&key("Player1"), &key("Player2")));
    }

    #[test]
    fn credits_the_name_shown_while_the_line_was_spoken() {
        let mut t = Tracker::default();
        let t0 = Instant::now();
        let ms = |m: u64| t0 + Duration::from_millis(m);
        // Player One talks from 0 to 1.5 s, Боец_7 from 3 to 4.5 s.
        for m in (0..=1500).step_by(300) {
            t.observe(ms(m), &lines(&["Player One"]));
        }
        t.observe(ms(2200), &[]);
        for m in (3000..=4500).step_by(300) {
            t.observe(ms(m), &lines(&["Боец_7"]));
        }
        assert_eq!(
            t.speaker_between(ms(100), ms(1400)),
            Some("Player One".into())
        );
        assert_eq!(t.speaker_between(ms(3100), ms(4400)), Some("Боец_7".into()));
        // Nothing shown at all: no name.
        assert_eq!(t.speaker_between(ms(10_000), ms(11_000)), None);
    }

    #[test]
    fn a_chat_sender_is_trusted_from_the_first_reading() {
        let mut t = Tracker::default();
        let t0 = Instant::now();
        t.confirm("Player One");
        t.confirm("[ALL] x: y"); // not a name
        t.observe(t0, &lines(&["Player Onе"])); // Cyrillic е, read once
        assert_eq!(t.speaker_between(t0, t0).as_deref(), Some("Player One"));
    }

    #[test]
    fn no_guess_when_two_people_talk_together() {
        let mut t = Tracker::default();
        let t0 = Instant::now();
        for m in 0..6u64 {
            t.observe(
                t0 + Duration::from_millis(m * 300),
                &lines(&["Player One", "Боец_7"]),
            );
        }
        assert_eq!(
            t.speaker_between(t0, t0 + Duration::from_millis(1500)),
            None
        );
        // One read once, never confirmed: not credited.
        let mut t = Tracker::default();
        t.observe(t0, &lines(&["Ghost"]));
        assert_eq!(t.speaker_between(t0, t0), None);
    }

    #[test]
    fn old_readings_and_extra_names_are_forgotten() {
        let mut t = Tracker::default();
        let t0 = Instant::now();
        t.observe(t0, &lines(&["Player One"]));
        t.observe(t0 + Duration::from_secs(40), &lines(&["Player One"]));
        assert_eq!(t.sightings.len(), 1);
        for i in 0..40 {
            t.observe(
                t0 + Duration::from_secs(41),
                &lines(&[&format!("Name{i}x")]),
            );
        }
        assert!(t.known.len() <= MAX_KNOWN);
        t.clear();
        assert!(t.known().is_empty());
    }
}
