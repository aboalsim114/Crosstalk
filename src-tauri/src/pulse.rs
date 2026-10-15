//! Pipeline pulse (issue #103): while a capture runs, one journal line every
//! 30 s says what each stage did — sound received from CS2, segments found by
//! the VAD, transcripts kept or rejected by Whisper, lines translated — and a
//! warning names the stage that stops, instead of a silent failure.
//!
//! Only counters and levels: never a transcript, a sound or a player name.
//! It listens to the events the services already send to the window, like
//! the journal (`logs::watch`), so the services know nothing of it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::speech::{RejectReason, SpeechStats};

/// Length of one summary window.
pub const WINDOW_MS: u64 = 30_000;
/// Below this peak the capture is silence (digital zero or near).
const SILENT_PEAK_DB: f32 = -60.0;
/// A level reading above this counts as "sound" (voices, game).
const SOUND_RMS_DB: f32 = -50.0;
/// Sound without a single segment for this long: the VAD hears no speech.
const NO_SPEECH_AFTER_MS: u64 = 120_000;
/// Segments sent to Whisper without one kept transcript before warning.
const WHISPER_SILENT_AFTER: u64 = 3;
/// Distinct translation errors written per capture (the rest are counted).
const MAX_ERRORS_LOGGED: usize = 5;

/// Reasons that leave a line in its language on purpose (not an error).
const QUIET: [&str; 4] = [
    "traduction désactivée",
    "aucun moteur de traduction connecté",
    "langue non gérée",
    "langue non choisie",
];

/// A journal line: level and message (source "pipeline").
pub type Entry = (&'static str, String);

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PulseView {
    pub capturing: bool,
    /// Last 30 s summary of this capture.
    pub summary: Option<String>,
    /// Warnings raised during this capture, oldest first.
    pub alerts: Vec<String>,
}

#[derive(Default)]
struct Window {
    start: u64,
    levels: u32,
    rms_sum: f64,
    peak_db: Option<f32>,
    sound_readings: u32,
    stats_base: SpeechStats,
    tr_ok: u32,
    tr_quiet: u32,
    tr_err: u32,
    last_err: Option<String>,
}

#[derive(Default)]
pub struct Pulse {
    capturing: bool,
    window: Window,
    /// Latest cumulative speech counters.
    stats: SpeechStats,
    /// Counters when this capture started.
    capture_base: SpeechStats,
    /// Time with sound received during this capture, in ms.
    sound_ms: u64,
    last_level_at: Option<u64>,
    raised: BTreeSet<&'static str>,
    errors_logged: BTreeSet<String>,
    view: PulseView,
}

fn db(linear: f64) -> f32 {
    if linear <= 1e-9 {
        -120.0
    } else {
        (20.0 * linear.log10()) as f32
    }
}

fn reason(r: RejectReason) -> &'static str {
    match r {
        RejectReason::Short => "trop court",
        RejectReason::Transient => "tir/explosion",
        RejectReason::Stale => "trop tard",
        RejectReason::Overflow => "file pleine",
        RejectReason::Empty => "vide",
        RejectReason::NoSpeech => "pas de parole",
        RejectReason::LowConfidence => "confiance faible",
        RejectReason::Hallucination => "hallucination",
        RejectReason::Repetition => "boucle",
        RejectReason::WrongScript => "mauvaise écriture",
    }
}

/// Rejections decided before Whisper (by the VAD's segmenter or the queue).
fn before_whisper(r: RejectReason) -> bool {
    matches!(
        r,
        RejectReason::Short
            | RejectReason::Transient
            | RejectReason::Stale
            | RejectReason::Overflow
    )
}

/// Counters of `now` minus `base` (a reset counts from zero).
fn delta(now: &SpeechStats, base: &SpeechStats) -> SpeechStats {
    let mut rejected = BTreeMap::new();
    for (r, n) in &now.rejected {
        let d = n.saturating_sub(base.rejected.get(r).copied().unwrap_or(0));
        if d > 0 {
            rejected.insert(*r, d);
        }
    }
    SpeechStats {
        segments: now.segments.saturating_sub(base.segments),
        transcribed: now.transcribed.saturating_sub(base.transcribed),
        rejected,
        dropped_blocks: now.dropped_blocks.saturating_sub(base.dropped_blocks),
        ..Default::default()
    }
}

fn list(
    reasons: &BTreeMap<RejectReason, u64>,
    pick: impl Fn(RejectReason) -> bool,
) -> (u64, String) {
    let picked: Vec<(RejectReason, u64)> = reasons
        .iter()
        .filter(|(r, _)| pick(**r))
        .map(|(r, n)| (*r, *n))
        .collect();
    let total = picked.iter().map(|(_, n)| n).sum();
    let text = picked
        .iter()
        .map(|(r, n)| format!("{} {n}", reason(*r)))
        .collect::<Vec<_>>()
        .join(", ");
    (total, text)
}

fn with_reasons(n: u64, text: &str) -> String {
    if text.is_empty() {
        n.to_string()
    } else {
        format!("{n} ({text})")
    }
}

impl Pulse {
    pub fn view(&self) -> PulseView {
        PulseView {
            capturing: self.capturing,
            ..self.view.clone()
        }
    }

    /// The capture started (`true`) or stopped. A stop writes the summary of
    /// the last, partial window when it lasted 5 s or more.
    pub fn capture(&mut self, capturing: bool, now: u64) -> Vec<Entry> {
        let mut out = Vec::new();
        if capturing && !self.capturing {
            self.capturing = true;
            self.capture_base = self.stats.clone();
            self.sound_ms = 0;
            self.last_level_at = None;
            self.raised.clear();
            self.errors_logged.clear();
            self.view = PulseView::default();
            self.new_window(now);
        } else if !capturing && self.capturing {
            if now.saturating_sub(self.window.start) >= 5_000 {
                out = self.close_window(now);
            }
            self.capturing = false;
        }
        out
    }

    pub fn level(&mut self, rms_db: f32, peak_db: f32, now: u64) {
        if !self.capturing {
            return;
        }
        let w = &mut self.window;
        w.levels += 1;
        w.rms_sum += 10f64.powf(rms_db as f64 / 20.0);
        w.peak_db = Some(w.peak_db.map_or(peak_db, |p| p.max(peak_db)));
        if rms_db > SOUND_RMS_DB {
            w.sound_readings += 1;
            if let Some(prev) = self.last_level_at {
                self.sound_ms += now.saturating_sub(prev).min(1_000);
            }
        }
        self.last_level_at = Some(now);
    }

    pub fn stats(&mut self, stats: SpeechStats) {
        self.stats = stats;
    }

    /// A final translation result (live ones are ignored). The first time
    /// each error is seen in a capture it is written at once.
    pub fn translation(&mut self, translated: bool, error: Option<&str>) -> Vec<Entry> {
        if !self.capturing {
            return Vec::new();
        }
        let w = &mut self.window;
        match error {
            _ if translated => w.tr_ok += 1,
            Some(e) if QUIET.iter().any(|q| e.starts_with(q)) => w.tr_quiet += 1,
            e => {
                let e: String = e
                    .unwrap_or("moteur indisponible")
                    .chars()
                    .take(160)
                    .collect();
                w.tr_err += 1;
                w.last_err = Some(e.clone());
                if self.errors_logged.len() < MAX_ERRORS_LOGGED
                    && self.errors_logged.insert(e.clone())
                {
                    return vec![("WARN", format!("traduction impossible : {e}"))];
                }
            }
        }
        Vec::new()
    }

    /// Every second or so: the summary and warnings once a window is over.
    pub fn tick(&mut self, now: u64) -> Vec<Entry> {
        if self.capturing && now.saturating_sub(self.window.start) >= WINDOW_MS {
            self.close_window(now)
        } else {
            Vec::new()
        }
    }

    fn new_window(&mut self, now: u64) {
        self.window = Window {
            start: now,
            stats_base: self.stats.clone(),
            ..Default::default()
        };
    }

    fn raise(&mut self, out: &mut Vec<Entry>, key: &'static str, msg: String) {
        if self.raised.insert(key) {
            self.view.alerts.push(msg.clone());
            out.push(("WARN", msg));
        }
    }

    fn close_window(&mut self, now: u64) -> Vec<Entry> {
        let mut out = Vec::new();
        let secs = (now.saturating_sub(self.window.start) + 500) / 1000;
        let w = std::mem::take(&mut self.window);
        let d = delta(&self.stats, &w.stats_base);
        let total = delta(&self.stats, &self.capture_base);

        // Sound received from CS2.
        let sound = if w.levels == 0 {
            "aucun son reçu".to_string()
        } else {
            format!(
                "son moyen {:.0} dBFS, crête {:.0} dBFS",
                db(w.rms_sum / w.levels as f64),
                w.peak_db.unwrap_or(-120.0)
            )
        };
        // VAD: segments, and those dropped before Whisper.
        let (early, early_why) = list(&d.rejected, before_whisper);
        let (late, late_why) = list(&d.rejected, |r| !before_whisper(r));
        let vad = format!(
            "VAD : {} segment(s), écartés avant Whisper : {}",
            d.segments,
            with_reasons(early, &early_why)
        );
        let asr = format!(
            "Whisper : {} gardée(s), rejetées : {}",
            d.transcribed,
            with_reasons(late, &late_why)
        );
        let tr = format!(
            "traduction : {} ok, {} laissée(s) telle(s) quelle(s), {} erreur(s)",
            w.tr_ok, w.tr_quiet, w.tr_err
        );
        let summary = format!("{secs} s de capture : {sound} · {vad} · {asr} · {tr}");
        self.view.summary = Some(summary.clone());
        out.push(("INFO", summary));

        // The stage that stops.
        if w.levels == 0 {
            self.raise(
                &mut out,
                "no-audio",
                "capture : aucun bloc audio reçu de CS2 pendant la capture (flux interrompu)"
                    .into(),
            );
        } else if w.peak_db.unwrap_or(-120.0) < SILENT_PEAK_DB {
            self.raise(
                &mut out,
                "silent",
                format!(
                    "capture silencieuse : CS2 n'envoie que du silence (crête {:.0} dBFS) — vérifiez que le son de CS2 et le chat vocal sont audibles sur ce PC",
                    w.peak_db.unwrap_or(-120.0)
                ),
            );
        }
        if total.segments == 0 && self.sound_ms >= NO_SPEECH_AFTER_MS {
            self.raise(
                &mut out,
                "no-speech",
                format!(
                    "du son arrive de CS2 depuis {} s mais la VAD n'a trouvé aucune parole (seuil VAD trop haut, ou voix absentes du son capturé)",
                    self.sound_ms / 1000
                ),
            );
        }
        let (total_early, total_early_why) = list(&total.rejected, before_whisper);
        let sent = total.segments.saturating_sub(total_early);
        if total.segments >= WHISPER_SILENT_AFTER && sent == 0 {
            self.raise(
                &mut out,
                "all-early",
                format!(
                    "{} segment(s) trouvés par la VAD, tous écartés avant Whisper ({total_early_why})",
                    total.segments
                ),
            );
        }
        if sent >= WHISPER_SILENT_AFTER && total.transcribed == 0 {
            let (_, why) = list(&total.rejected, |r| !before_whisper(r));
            self.raise(
                &mut out,
                "whisper-nothing",
                format!(
                    "{sent} segment(s) envoyés à Whisper, aucune transcription gardée{}",
                    if why.is_empty() {
                        " (aucun résultat : Whisper bloqué ?)".to_string()
                    } else {
                        format!(" (rejets : {why})")
                    }
                ),
            );
        }
        if d.dropped_blocks > 0 {
            self.raise(
                &mut out,
                "dropped",
                format!(
                    "{} bloc(s) audio perdus : la reconnaissance ne suit pas (processeur trop chargé ?)",
                    d.dropped_blocks
                ),
            );
        }
        if w.tr_err > 0 && w.tr_ok == 0 {
            self.raise(
                &mut out,
                "translation",
                format!(
                    "traduction : aucune ligne traduite, {} erreur(s) (dernière : {})",
                    w.tr_err,
                    w.last_err.as_deref().unwrap_or("?")
                ),
            );
        }
        self.new_window(now);
        out
    }
}

// ---------- Tauri glue ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Level {
    rms_db: f32,
    peak_db: f32,
}

#[derive(Deserialize)]
struct Status {
    state: String,
}

#[derive(Deserialize)]
struct Result_ {
    translated: Option<String>,
    error: Option<String>,
    #[serde(default)]
    partial: bool,
}

fn write(entries: Vec<Entry>) {
    for (level, msg) in entries {
        crate::logs::global().log(level, "pipeline", &msg);
    }
}

pub type Shared = Arc<Mutex<Pulse>>;

/// Listens to the services' events and writes the pulse into the journal.
pub fn watch(app: &tauri::AppHandle) -> Shared {
    use tauri::Listener;
    let pulse: Shared = Arc::default();
    let now = crate::history::now_ms;

    let p = pulse.clone();
    app.listen_any(crate::audio::STATUS_EVENT, move |e| {
        if let Ok(s) = serde_json::from_str::<Status>(e.payload()) {
            write(p.lock().unwrap().capture(s.state == "capturing", now()));
        }
    });
    let p = pulse.clone();
    app.listen_any(crate::audio::LEVEL_EVENT, move |e| {
        if let Ok(l) = serde_json::from_str::<Level>(e.payload()) {
            p.lock().unwrap().level(l.rms_db, l.peak_db, now());
        }
    });
    let p = pulse.clone();
    app.listen_any(crate::speech::STATS_EVENT, move |e| {
        if let Ok(s) = serde_json::from_str::<SpeechStats>(e.payload()) {
            p.lock().unwrap().stats(s);
        }
    });
    let p = pulse.clone();
    app.listen_any(crate::translation::RESULT_EVENT, move |e| {
        if let Ok(r) = serde_json::from_str::<Result_>(e.payload()) {
            if !r.partial {
                write(
                    p.lock()
                        .unwrap()
                        .translation(r.translated.is_some(), r.error.as_deref()),
                );
            }
        }
    });
    let p = pulse.clone();
    let _ = std::thread::Builder::new()
        .name("pipeline-pulse".into())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            let entries = p.lock().unwrap().tick(now());
            write(entries);
        });
    pulse
}

/// The pulse of the running capture, for the Diagnostic screen.
#[tauri::command]
pub fn diag_pulse(pulse: tauri::State<'_, Shared>) -> PulseView {
    pulse.lock().unwrap().view()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(segments: u64, transcribed: u64, rejected: &[(RejectReason, u64)]) -> SpeechStats {
        SpeechStats {
            segments,
            transcribed,
            rejected: rejected.iter().copied().collect(),
            ..Default::default()
        }
    }

    /// 30 s of level readings at 15 per second.
    fn sound(p: &mut Pulse, from: u64, rms_db: f32, peak_db: f32) -> u64 {
        let mut t = from;
        for _ in 0..450 {
            t += 66;
            p.level(rms_db, peak_db, t);
        }
        t
    }

    fn warnings(out: &[Entry]) -> Vec<&str> {
        out.iter()
            .filter(|(l, _)| *l == "WARN")
            .map(|(_, m)| m.as_str())
            .collect()
    }

    #[test]
    fn a_healthy_window_is_one_summary_line_per_stage_without_warning() {
        let mut p = Pulse::default();
        p.stats(stats(10, 4, &[(RejectReason::Transient, 1)]));
        assert!(p.capture(true, 0).is_empty());
        sound(&mut p, 0, -30.0, -6.0);
        p.stats(stats(
            15,
            7,
            &[
                (RejectReason::Transient, 2),
                (RejectReason::Hallucination, 1),
            ],
        ));
        assert!(p.translation(true, None).is_empty());
        assert!(p
            .translation(false, Some("langue non choisie (Langues)"))
            .is_empty());
        assert!(p.tick(29_000).is_empty());
        let out = p.tick(30_000);
        assert_eq!(out.len(), 1, "{out:?}");
        let (level, line) = &out[0];
        assert_eq!(*level, "INFO");
        assert!(
            line.starts_with("30 s de capture : son moyen -30 dBFS, crête -6 dBFS"),
            "{line}"
        );
        assert!(
            line.contains("VAD : 5 segment(s), écartés avant Whisper : 1 (tir/explosion 1)"),
            "{line}"
        );
        assert!(
            line.contains("Whisper : 3 gardée(s), rejetées : 1 (hallucination 1)"),
            "{line}"
        );
        assert!(
            line.contains("traduction : 1 ok, 1 laissée(s) telle(s) quelle(s), 0 erreur(s)"),
            "{line}"
        );
        assert_eq!(p.view().summary.as_deref(), Some(line.as_str()));
        assert!(p.view().alerts.is_empty());
    }

    #[test]
    fn no_audio_block_and_silence_are_named() {
        let mut p = Pulse::default();
        p.capture(true, 0);
        let out = p.tick(30_000);
        assert!(out[0].1.contains("aucun son reçu"), "{out:?}");
        assert_eq!(warnings(&out).len(), 1);
        assert!(warnings(&out)[0].contains("aucun bloc audio reçu"));

        let mut p = Pulse::default();
        p.capture(true, 0);
        sound(&mut p, 0, -96.0, -90.0);
        let out = p.tick(30_000);
        assert!(
            warnings(&out)[0].starts_with("capture silencieuse"),
            "{out:?}"
        );
        // Raised once per capture, not every 30 s.
        sound(&mut p, 30_000, -96.0, -90.0);
        assert!(warnings(&p.tick(60_000)).is_empty());
        assert_eq!(p.view().alerts.len(), 1);
    }

    #[test]
    fn sound_without_any_segment_for_two_minutes_blames_the_vad() {
        let mut p = Pulse::default();
        p.capture(true, 0);
        let mut t = 0;
        let mut all = Vec::new();
        // 4 windows of 29.7 s of sound: not yet 2 minutes; the 5th warns.
        for i in 1..=5 {
            t = sound(&mut p, t, -25.0, -5.0);
            all.extend(p.tick(i * WINDOW_MS));
        }
        let w = warnings(&all);
        assert_eq!(w.len(), 1, "{all:?}");
        assert!(w[0].contains("la VAD n'a trouvé aucune parole"), "{}", w[0]);
    }

    #[test]
    fn segments_all_rejected_by_whisper_or_before_it_are_named_with_their_reasons() {
        let mut p = Pulse::default();
        p.capture(true, 0);
        sound(&mut p, 0, -25.0, -5.0);
        p.stats(stats(
            4,
            0,
            &[(RejectReason::Hallucination, 3), (RejectReason::Short, 1)],
        ));
        let w = p.tick(30_000);
        assert_eq!(
            warnings(&w),
            ["3 segment(s) envoyés à Whisper, aucune transcription gardée (rejets : hallucination 3)"]
        );

        let mut p = Pulse::default();
        p.capture(true, 0);
        sound(&mut p, 0, -25.0, -5.0);
        p.stats(stats(5, 0, &[(RejectReason::Transient, 5)]));
        assert_eq!(
            warnings(&p.tick(30_000)),
            ["5 segment(s) trouvés par la VAD, tous écartés avant Whisper (tir/explosion 5)"]
        );

        // Sent but nothing came back at all.
        let mut p = Pulse::default();
        p.capture(true, 0);
        sound(&mut p, 0, -25.0, -5.0);
        p.stats(stats(3, 0, &[]));
        assert!(warnings(&p.tick(30_000))[0].contains("Whisper bloqué"));
    }

    #[test]
    fn counters_from_before_the_capture_do_not_count() {
        let mut p = Pulse::default();
        p.stats(stats(50, 0, &[(RejectReason::Hallucination, 50)]));
        p.capture(true, 0);
        sound(&mut p, 0, -25.0, -5.0);
        p.stats(stats(51, 1, &[(RejectReason::Hallucination, 50)]));
        let out = p.tick(30_000);
        assert!(warnings(&out).is_empty(), "{out:?}");
        assert!(out[0].1.contains("VAD : 1 segment(s)"));
    }

    #[test]
    fn translation_errors_are_written_once_each_then_summarised() {
        let mut p = Pulse::default();
        assert!(
            p.translation(false, Some("LLM : clé refusée")).is_empty(),
            "ignored outside a capture"
        );
        p.capture(true, 0);
        sound(&mut p, 0, -25.0, -5.0);
        assert_eq!(
            p.translation(false, Some("LLM : clé refusée")),
            [(
                "WARN",
                "traduction impossible : LLM : clé refusée".to_string()
            )]
        );
        assert!(p.translation(false, Some("LLM : clé refusée")).is_empty());
        assert!(p
            .translation(
                false,
                Some("aucun moteur de traduction connecté (Audio & IA)")
            )
            .is_empty());
        let out = p.tick(30_000);
        assert!(
            out[0]
                .1
                .contains("0 ok, 1 laissée(s) telle(s) quelle(s), 2 erreur(s)"),
            "{out:?}"
        );
        assert_eq!(
            warnings(&out),
            ["traduction : aucune ligne traduite, 2 erreur(s) (dernière : LLM : clé refusée)"]
        );
    }

    #[test]
    fn dropped_blocks_and_a_stop_write_the_last_partial_window() {
        let mut p = Pulse::default();
        p.capture(true, 0);
        sound(&mut p, 0, -25.0, -5.0);
        p.stats(SpeechStats {
            dropped_blocks: 12,
            ..Default::default()
        });
        let out = p.capture(false, 20_000);
        assert!(out[0].1.starts_with("20 s de capture"), "{out:?}");
        assert!(warnings(&out)[0].starts_with("12 bloc(s) audio perdus"));
        assert!(!p.view().capturing);
        // A very short capture writes nothing.
        p.capture(true, 30_000);
        assert!(p.capture(false, 32_000).is_empty());
        assert!(p.tick(100_000).is_empty());
    }
}
