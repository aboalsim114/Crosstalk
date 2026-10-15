//! Automatic push-to-talk for the AI voice: once the sentence is synthesised,
//! hold CS2's voice key, play the sentence into the cable, release the key
//! when the cable has really played it (plus a short margin).
//!
//! The key is sent with `SendInput`, the documented Windows API for
//! synthetic input, from Crosstalk's own process: nothing is injected into
//! CS2, its memory is never read or written. Only while CS2 is in front.
//!
//! States: Idle → Synthesized → PttHeld → Playing → Releasing → Idle. The key
//! is released exactly once whatever happens (end, error, cancel, focus lost,
//! settings changed, Crosstalk closing): `Held` releases it on drop.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

/// Delay between key down and the first sample: CS2 opens its microphone.
pub const DEFAULT_LEAD_MS: u32 = 250;
/// Kept after the last sample has left the cable, not to cut the last syllable.
pub const DEFAULT_TAIL_MS: u32 = 350;
pub const LEAD_RANGE: (u32, u32) = (50, 1000);
pub const TAIL_RANGE: (u32, u32) = (100, 1500);
/// Beyond the sentence's length, the release happens anyway (stuck output).
const SAFETY_MARGIN: Duration = Duration::from_secs(3);
const TICK: Duration = Duration::from_millis(10);

/// A key or mouse button CS2 can bind to voice chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PttKey {
    /// Windows virtual-key code (letters, digits, F1–F12, modifiers…).
    Key(u16),
    /// Middle (3), side buttons (4, 5).
    Mouse(u8),
}

/// "K", "v", "F5", "CapsLock", "LAlt", "Mouse4"… (CS2's bind names when they exist).
pub fn parse_key(name: &str) -> Result<PttKey, String> {
    let n = name.trim();
    let up = n.to_ascii_uppercase();
    let vk = match up.as_str() {
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => s.as_bytes()[0] as u16,
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "CAPSLOCK" | "CAPS" => 0x14,
        "SHIFT" | "LSHIFT" => 0xA0,
        "RSHIFT" => 0xA1,
        "CTRL" | "LCTRL" => 0xA2,
        "RCTRL" => 0xA3,
        "ALT" | "LALT" => 0xA4,
        "RALT" => 0xA5,
        "MOUSE3" => return Ok(PttKey::Mouse(3)),
        "MOUSE4" => return Ok(PttKey::Mouse(4)),
        "MOUSE5" => return Ok(PttKey::Mouse(5)),
        s if s.starts_with('F') => match s[1..].parse::<u16>() {
            Ok(i @ 1..=12) => 0x70 + i - 1,
            _ => return Err(format!("touche inconnue : « {n} »")),
        },
        _ => return Err(format!("touche inconnue : « {n} »")),
    };
    Ok(PttKey::Key(vk))
}

/// Settings of one transmission (from the voice options).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// None: no automatic press (open microphone, or held by hand).
    pub key: Option<PttKey>,
    pub lead: Duration,
    pub tail: Duration,
}

impl Plan {
    pub fn new(auto: bool, key: &str, lead_ms: u32, tail_ms: u32) -> Result<Self, String> {
        Ok(Self {
            key: if auto { Some(parse_key(key)?) } else { None },
            lead: Duration::from_millis(lead_ms.clamp(LEAD_RANGE.0, LEAD_RANGE.1) as u64),
            tail: Duration::from_millis(tail_ms.clamp(TAIL_RANGE.0, TAIL_RANGE.1) as u64),
        })
    }
}

/// Where the transmission is, for the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Idle,
    /// Translation and synthesis running ("Préparation voix").
    Preparing,
    /// Sentence ready, nothing pressed yet.
    Synthesized,
    /// Key held, waiting for CS2 to open its microphone ("Activation micro").
    Held,
    /// Sentence going into the cable ("Transmission").
    Playing,
    Releasing,
    Done,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PttStatus {
    pub phase: Phase,
    /// What went wrong, or why the key was not pressed (shown as is).
    pub detail: Option<String>,
    /// The key was pressed by Crosstalk for this sentence.
    pub pressed: bool,
}

impl PttStatus {
    pub fn at(phase: Phase) -> Self {
        Self {
            phase,
            detail: None,
            pressed: false,
        }
    }
}

/// Keyboard side, behind a trait for the tests.
pub trait Keys: Send + Sync {
    fn press(&self, key: PttKey) -> Result<(), String>;
    fn release(&self, key: PttKey) -> Result<(), String>;
    /// The user already holds this key (then Crosstalk neither presses nor releases it).
    fn held_by_user(&self, key: PttKey) -> bool;
    /// CS2 has the keyboard focus.
    fn game_in_front(&self) -> bool;
}

/// Audio side: the cable feed.
pub trait Playback: Send + Sync {
    fn play(&self, samples: &[f32], rate: u32);
    /// Every sample queued has left the cable (device buffer included).
    fn played(&self) -> bool;
    /// Drops what is still queued (cancel).
    fn stop(&self);
}

/// The key Crosstalk holds right now, released once: by `release`, on drop,
/// or by `release_held` from another thread (Crosstalk closing).
pub struct Held {
    key: PttKey,
    keys: Arc<dyn Keys>,
    slot: Arc<Mutex<Option<PttKey>>>,
}

impl Held {
    fn press(
        keys: Arc<dyn Keys>,
        key: PttKey,
        slot: Arc<Mutex<Option<PttKey>>>,
    ) -> Result<Self, String> {
        let mut s = slot.lock().unwrap();
        keys.press(key)?;
        *s = Some(key);
        drop(s);
        Ok(Self { key, keys, slot })
    }

    fn release(self) {
        drop(self)
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        let mut s = self.slot.lock().unwrap();
        // Already released from elsewhere: nothing more.
        if s.take().is_some() {
            let _ = self.keys.release(self.key);
        }
    }
}

/// Releases the key held by a transmission, if any (Crosstalk closing).
pub fn release_held(keys: &dyn Keys, slot: &Mutex<Option<PttKey>>) {
    if let Some(k) = slot.lock().unwrap().take() {
        let _ = keys.release(k);
    }
}

/// Plays one synthesised sentence with the plan; `cancelled` is polled every
/// 10 ms. Blocks until the key is released. Reports each phase.
#[allow(clippy::too_many_arguments)]
pub fn transmit(
    plan: Plan,
    samples: &[f32],
    rate: u32,
    keys: &Arc<dyn Keys>,
    playback: &dyn Playback,
    slot: &Arc<Mutex<Option<PttKey>>>,
    cancelled: &dyn Fn() -> bool,
    report: &mut dyn FnMut(PttStatus),
) -> PttStatus {
    report(PttStatus::at(Phase::Synthesized));
    let length = Duration::from_millis(samples.len() as u64 * 1000 / rate.max(1) as u64);
    let mut note = None;
    let mut held = None;
    if let Some(key) = plan.key {
        if cancelled() {
            return finish(Phase::Cancelled, None, false, report);
        }
        if !keys.game_in_front() {
            note = Some(
                "CS2 n'est pas au premier plan : touche non appuyée, la voix est jouée sans."
                    .to_string(),
            );
        } else if keys.held_by_user(key) {
            note = Some("touche déjà maintenue par vous : Crosstalk ne la touche pas.".to_string());
        } else {
            match Held::press(keys.clone(), key, slot.clone()) {
                Ok(h) => held = Some(h),
                Err(e) => {
                    note = Some(format!(
                        "appui automatique impossible ({e}) : maintenez la touche vous-même."
                    ))
                }
            }
        }
    }
    let pressed = held.is_some();
    if pressed {
        report(PttStatus {
            phase: Phase::Held,
            detail: None,
            pressed,
        });
        let until = Instant::now() + plan.lead;
        while Instant::now() < until {
            if let Some(stop) = interrupted(cancelled, keys.as_ref()) {
                drop(held);
                return finish(stop.0, stop.1, pressed, report);
            }
            std::thread::sleep(TICK);
        }
    }
    report(PttStatus {
        phase: Phase::Playing,
        detail: note.clone(),
        pressed,
    });
    playback.play(samples, rate);
    let deadline = Instant::now() + length + SAFETY_MARGIN;
    loop {
        if cancelled() {
            playback.stop();
            drop(held);
            return finish(Phase::Cancelled, None, pressed, report);
        }
        if pressed && !keys.game_in_front() {
            playback.stop();
            drop(held);
            return finish(
                Phase::Error,
                Some(
                    "CS2 n'est plus au premier plan : touche relâchée, phrase interrompue.".into(),
                ),
                pressed,
                report,
            );
        }
        if playback.played() || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(TICK);
    }
    if pressed {
        let until = Instant::now() + plan.tail;
        while Instant::now() < until && !cancelled() {
            std::thread::sleep(TICK);
        }
        report(PttStatus {
            phase: Phase::Releasing,
            detail: None,
            pressed,
        });
    }
    if let Some(h) = held {
        h.release();
    }
    finish(Phase::Done, note, pressed, report)
}

fn interrupted(cancelled: &dyn Fn() -> bool, keys: &dyn Keys) -> Option<(Phase, Option<String>)> {
    if cancelled() {
        return Some((Phase::Cancelled, None));
    }
    if !keys.game_in_front() {
        return Some((
            Phase::Error,
            Some("CS2 n'est plus au premier plan : touche relâchée avant la phrase.".into()),
        ));
    }
    None
}

fn finish(
    phase: Phase,
    detail: Option<String>,
    pressed: bool,
    report: &mut dyn FnMut(PttStatus),
) -> PttStatus {
    let s = PttStatus {
        phase,
        detail,
        pressed,
    };
    report(s.clone());
    s
}

/// Cancels every transmission started before the call (and the queued ones).
#[derive(Default)]
pub struct Generation(AtomicU64);

impl Generation {
    pub fn current(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
    pub fn bump(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }
}

// ---------- Windows ----------

/// `SendInput` (scan codes, as games read them), `GetAsyncKeyState`, and the
/// foreground window's process compared with cs2.exe by name.
#[cfg(windows)]
pub struct WindowsKeys {
    /// Last foreground PID found to be CS2 (the process list is read once per window change).
    cs2_pid: Mutex<Option<u32>>,
}

#[cfg(windows)]
impl WindowsKeys {
    pub fn new() -> Self {
        Self {
            cs2_pid: Mutex::new(None),
        }
    }

    fn send(&self, key: PttKey, down: bool) -> Result<(), String> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        let input = match key {
            PttKey::Key(vk) => {
                // SAFETY: plain Win32 call with a virtual-key code.
                let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
                let mut flags = KEYEVENTF_SCANCODE;
                // Right Ctrl / Alt are extended keys.
                if matches!(vk, 0xA3 | 0xA5) {
                    flags |= KEYEVENTF_EXTENDEDKEY;
                }
                if !down {
                    flags |= KEYEVENTF_KEYUP;
                }
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: scan,
                            dwFlags: flags,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }
            }
            PttKey::Mouse(b) => {
                let (flags, data) = match (b, down) {
                    (3, true) => (MOUSEEVENTF_MIDDLEDOWN, 0),
                    (3, false) => (MOUSEEVENTF_MIDDLEUP, 0),
                    (4, true) => (MOUSEEVENTF_XDOWN, 1),
                    (4, false) => (MOUSEEVENTF_XUP, 1),
                    (_, true) => (MOUSEEVENTF_XDOWN, 2),
                    (_, false) => (MOUSEEVENTF_XUP, 2),
                };
                INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: data,
                            dwFlags: flags,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }
            }
        };
        // SAFETY: one well-formed INPUT, its exact size.
        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        if sent == 1 {
            Ok(())
        } else {
            Err("Windows a refusé l'entrée (CS2 lancé en administrateur ?)".into())
        }
    }
}

#[cfg(windows)]
impl Default for WindowsKeys {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl Keys for WindowsKeys {
    fn press(&self, key: PttKey) -> Result<(), String> {
        self.send(key, true)
    }
    fn release(&self, key: PttKey) -> Result<(), String> {
        self.send(key, false)
    }
    fn held_by_user(&self, key: PttKey) -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        let vk = match key {
            PttKey::Key(vk) => vk as i32,
            PttKey::Mouse(3) => 0x04,
            PttKey::Mouse(4) => 0x05,
            PttKey::Mouse(_) => 0x06,
        };
        // SAFETY: plain Win32 call.
        (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
    }
    fn game_in_front(&self) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };
        let mut pid = 0u32;
        // SAFETY: plain Win32 calls; only the window's process id is read.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return false;
            }
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid == 0 {
            return false;
        }
        let mut cached = self.cs2_pid.lock().unwrap();
        if *cached == Some(pid) {
            return true;
        }
        let is_cs2 = crate::game::process::process_name(pid)
            .is_some_and(|n| n.eq_ignore_ascii_case(crate::game::process::CS2_EXE));
        if is_cs2 {
            *cached = Some(pid);
        }
        is_cs2
    }
}

/// Elsewhere (tests, Linux builds): no keyboard, never "in front".
#[cfg(not(windows))]
#[derive(Default)]
pub struct WindowsKeys;

#[cfg(not(windows))]
impl WindowsKeys {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(windows))]
impl Keys for WindowsKeys {
    fn press(&self, _: PttKey) -> Result<(), String> {
        Err("appui automatique disponible sous Windows".into())
    }
    fn release(&self, _: PttKey) -> Result<(), String> {
        Ok(())
    }
    fn held_by_user(&self, _: PttKey) -> bool {
        false
    }
    fn game_in_front(&self) -> bool {
        false
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    /// What happened, in order, with the time since the start.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Ev {
        Down(PttKey),
        Up(PttKey),
        Play(usize),
        Stop,
    }

    #[derive(Default)]
    pub struct FakeKeys {
        pub log: Mutex<Vec<(Ev, Duration)>>,
        pub start: Mutex<Option<Instant>>,
        pub behind: AtomicBool,
        pub user_holds: AtomicBool,
        pub refuse: AtomicBool,
    }

    impl FakeKeys {
        pub fn push(&self, e: Ev) {
            let start = *self.start.lock().unwrap().get_or_insert_with(Instant::now);
            self.log.lock().unwrap().push((e, start.elapsed()));
        }
        pub fn events(&self) -> Vec<Ev> {
            self.log
                .lock()
                .unwrap()
                .iter()
                .map(|(e, _)| e.clone())
                .collect()
        }
        pub fn at(&self, e: &Ev) -> Duration {
            self.log
                .lock()
                .unwrap()
                .iter()
                .find(|(x, _)| x == e)
                .unwrap()
                .1
        }
    }

    impl Keys for FakeKeys {
        fn press(&self, key: PttKey) -> Result<(), String> {
            if self.refuse.load(Ordering::SeqCst) {
                return Err("refusé".into());
            }
            self.push(Ev::Down(key));
            Ok(())
        }
        fn release(&self, key: PttKey) -> Result<(), String> {
            self.push(Ev::Up(key));
            Ok(())
        }
        fn held_by_user(&self, _: PttKey) -> bool {
            self.user_holds.load(Ordering::SeqCst)
        }
        fn game_in_front(&self) -> bool {
            !self.behind.load(Ordering::SeqCst)
        }
    }

    /// Plays in real time: "played" `ms_extra` after the sentence's length
    /// (the device buffer), unless stopped.
    pub struct FakePlayback {
        pub keys: Arc<FakeKeys>,
        pub until: Mutex<Option<Instant>>,
        pub ms_extra: u64,
        pub plays: AtomicUsize,
    }

    impl FakePlayback {
        pub fn new(keys: Arc<FakeKeys>) -> Self {
            Self {
                keys,
                until: Mutex::new(None),
                ms_extra: 60,
                plays: AtomicUsize::new(0),
            }
        }
    }

    impl Playback for FakePlayback {
        fn play(&self, samples: &[f32], rate: u32) {
            self.plays.fetch_add(1, Ordering::SeqCst);
            self.keys.push(Ev::Play(samples.len()));
            let ms = samples.len() as u64 * 1000 / rate as u64 + self.ms_extra;
            *self.until.lock().unwrap() = Some(Instant::now() + Duration::from_millis(ms));
        }
        fn played(&self) -> bool {
            self.until
                .lock()
                .unwrap()
                .map_or(true, |t| Instant::now() >= t)
        }
        fn stop(&self) {
            self.keys.push(Ev::Stop);
            *self.until.lock().unwrap() = None;
        }
    }

    const K: PttKey = PttKey::Key(b'K' as u16);
    /// 200 ms of audio at 8 kHz.
    fn sentence() -> Vec<f32> {
        vec![0.1; 1600]
    }
    fn plan() -> Plan {
        Plan::new(true, "K", 80, 120).unwrap()
    }

    fn run(
        plan: Plan,
        keys: &Arc<FakeKeys>,
        cancel: &AtomicBool,
    ) -> (PttStatus, Vec<Phase>, Arc<Mutex<Option<PttKey>>>) {
        let k: Arc<dyn Keys> = keys.clone();
        let pb = FakePlayback::new(keys.clone());
        let slot = Arc::new(Mutex::new(None));
        let mut phases = Vec::new();
        keys.push(Ev::Stop); // time origin
        keys.log.lock().unwrap().clear();
        let end = transmit(
            plan,
            &sentence(),
            8000,
            &k,
            &pb,
            &slot,
            &|| cancel.load(Ordering::SeqCst),
            &mut |s| phases.push(s.phase),
        );
        (end, phases, slot)
    }

    #[test]
    fn keys_are_parsed_like_cs2_binds() {
        assert_eq!(parse_key("K"), Ok(PttKey::Key(0x4B)));
        assert_eq!(parse_key(" v "), Ok(PttKey::Key(0x56)));
        assert_eq!(parse_key("4"), Ok(PttKey::Key(0x34)));
        assert_eq!(parse_key("F5"), Ok(PttKey::Key(0x74)));
        assert_eq!(parse_key("capslock"), Ok(PttKey::Key(0x14)));
        assert_eq!(parse_key("LAlt"), Ok(PttKey::Key(0xA4)));
        assert_eq!(parse_key("Mouse4"), Ok(PttKey::Mouse(4)));
        for bad in ["", "KK", "F13", "F0", "Échap", "mouse9"] {
            assert!(parse_key(bad).is_err(), "{bad}");
        }
        let p = Plan::new(true, "K", 5, 99_999).unwrap();
        assert_eq!(
            (p.lead, p.tail),
            (Duration::from_millis(50), Duration::from_millis(1500))
        );
        assert_eq!(Plan::new(false, "??", 0, 0).unwrap().key, None);
        assert!(Plan::new(true, "??", 0, 0).is_err());
    }

    #[test]
    fn key_down_then_lead_then_audio_then_drain_and_tail_then_one_key_up() {
        let keys = Arc::new(FakeKeys::default());
        let (end, phases, slot) = run(plan(), &keys, &AtomicBool::new(false));
        assert_eq!(end.phase, Phase::Done);
        assert!(end.pressed && end.detail.is_none());
        assert_eq!(keys.events(), vec![Ev::Down(K), Ev::Play(1600), Ev::Up(K)]);
        assert_eq!(
            phases,
            vec![
                Phase::Synthesized,
                Phase::Held,
                Phase::Playing,
                Phase::Releasing,
                Phase::Done
            ]
        );
        let down = keys.at(&Ev::Down(K));
        let play = keys.at(&Ev::Play(1600));
        let up = keys.at(&Ev::Up(K));
        // The microphone opens before the first sample…
        assert!(
            play - down >= Duration::from_millis(80),
            "{:?}",
            play - down
        );
        // …and stays open for the sentence, the device buffer and the margin.
        assert!(
            up - play >= Duration::from_millis(200 + 60 + 120),
            "{:?}",
            up - play
        );
        assert!(up - play < Duration::from_millis(900), "{:?}", up - play);
        assert!(slot.lock().unwrap().is_none());
    }

    #[test]
    fn nothing_is_pressed_when_off_behind_cs2_held_by_the_user_or_refused() {
        // Off (open microphone): the voice plays, no key.
        let keys = Arc::new(FakeKeys::default());
        let (end, phases, _) = run(
            Plan::new(false, "K", 80, 120).unwrap(),
            &keys,
            &AtomicBool::new(false),
        );
        assert_eq!((end.phase, end.pressed), (Phase::Done, false));
        assert_eq!(keys.events(), vec![Ev::Play(1600)]);
        assert!(!phases.contains(&Phase::Held));
        // CS2 not in front.
        let keys = Arc::new(FakeKeys::default());
        keys.behind.store(true, Ordering::SeqCst);
        let (end, ..) = run(plan(), &keys, &AtomicBool::new(false));
        assert_eq!(keys.events(), vec![Ev::Play(1600)]);
        assert!(end.detail.unwrap().contains("premier plan"));
        // The user already holds K: neither pressed nor released by Crosstalk.
        let keys = Arc::new(FakeKeys::default());
        keys.user_holds.store(true, Ordering::SeqCst);
        let (end, ..) = run(plan(), &keys, &AtomicBool::new(false));
        assert_eq!(keys.events(), vec![Ev::Play(1600)]);
        assert!(end.detail.unwrap().contains("déjà maintenue"));
        // Windows refuses the input: said clearly, the user can hold it.
        let keys = Arc::new(FakeKeys::default());
        keys.refuse.store(true, Ordering::SeqCst);
        let (end, ..) = run(plan(), &keys, &AtomicBool::new(false));
        assert_eq!(keys.events(), vec![Ev::Play(1600)]);
        assert!(end.detail.unwrap().contains("maintenez la touche"));
    }

    #[test]
    fn a_cancel_during_the_sentence_stops_it_and_releases_the_key_once() {
        let keys = Arc::new(FakeKeys::default());
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            c.store(true, Ordering::SeqCst);
        });
        let (end, _, slot) = run(plan(), &keys, &cancel);
        t.join().unwrap();
        assert_eq!(end.phase, Phase::Cancelled);
        assert_eq!(
            keys.events(),
            vec![Ev::Down(K), Ev::Play(1600), Ev::Stop, Ev::Up(K)]
        );
        assert!(slot.lock().unwrap().is_none());
    }

    #[test]
    fn a_cancel_before_the_audio_releases_without_playing() {
        let keys = Arc::new(FakeKeys::default());
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            c.store(true, Ordering::SeqCst);
        });
        let (end, ..) = run(plan(), &keys, &cancel);
        t.join().unwrap();
        assert_eq!(end.phase, Phase::Cancelled);
        assert_eq!(keys.events(), vec![Ev::Down(K), Ev::Up(K)]);
    }

    #[test]
    fn losing_cs2_focus_releases_the_key_and_stops_the_sentence() {
        let keys = Arc::new(FakeKeys::default());
        let k2 = keys.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            k2.behind.store(true, Ordering::SeqCst);
        });
        let (end, ..) = run(plan(), &keys, &AtomicBool::new(false));
        t.join().unwrap();
        assert_eq!(end.phase, Phase::Error);
        assert!(end.detail.unwrap().contains("plus au premier plan"));
        assert_eq!(
            keys.events(),
            vec![Ev::Down(K), Ev::Play(1600), Ev::Stop, Ev::Up(K)]
        );
    }

    #[test]
    fn closing_crosstalk_releases_the_held_key_and_the_transmission_does_not_release_it_again() {
        let keys = Arc::new(FakeKeys::default());
        let k: Arc<dyn Keys> = keys.clone();
        let slot = Arc::new(Mutex::new(None));
        let pb = Arc::new(FakePlayback::new(keys.clone()));
        let (k2, s2, pb2) = (k.clone(), slot.clone(), pb.clone());
        let t = std::thread::spawn(move || {
            let long = vec![0.1; 8000];
            transmit(plan(), &long, 8000, &k2, &*pb2, &s2, &|| false, &mut |_| {})
        });
        std::thread::sleep(Duration::from_millis(200));
        release_held(k.as_ref(), &slot);
        t.join().unwrap();
        let ups = keys
            .events()
            .iter()
            .filter(|e| matches!(e, Ev::Up(_)))
            .count();
        assert_eq!(ups, 1, "{:?}", keys.events());
    }

    #[test]
    fn a_panic_while_held_still_releases_the_key() {
        let keys = Arc::new(FakeKeys::default());
        let k: Arc<dyn Keys> = keys.clone();
        let slot = Arc::new(Mutex::new(None));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = Held::press(k.clone(), K, slot.clone()).unwrap();
            panic!("arrêt brutal");
        }));
        assert!(r.is_err());
        assert_eq!(keys.events(), vec![Ev::Down(K), Ev::Up(K)]);
    }

    #[test]
    fn a_stuck_output_never_keeps_the_key_forever() {
        struct Stuck;
        impl Playback for Stuck {
            fn play(&self, _: &[f32], _: u32) {}
            fn played(&self) -> bool {
                false
            }
            fn stop(&self) {}
        }
        let keys = Arc::new(FakeKeys::default());
        let k: Arc<dyn Keys> = keys.clone();
        let started = Instant::now();
        let end = transmit(
            plan(),
            &sentence(),
            8000,
            &k,
            &Stuck,
            &Arc::new(Mutex::new(None)),
            &|| false,
            &mut |_| {},
        );
        assert_eq!(end.phase, Phase::Done);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(keys.events(), vec![Ev::Down(K), Ev::Up(K)]);
    }

    /// The real Windows calls: SendInput accepts a down / up of F24 (a key no
    /// application uses) and no CS2 window is in front during the tests.
    #[cfg(windows)]
    #[test]
    fn windows_send_input_presses_and_releases_without_cs2_in_front() {
        let k = WindowsKeys::new();
        let f24 = PttKey::Key(0x87);
        assert_eq!(k.press(f24), Ok(()));
        assert_eq!(k.release(f24), Ok(()));
        assert!(!k.held_by_user(f24));
        assert!(!k.game_in_front());
    }

    #[test]
    fn generations_cancel_what_was_started_before() {
        let g = Generation::default();
        let first = g.current();
        assert_eq!(g.bump(), first + 1);
        assert_ne!(g.current(), first);
    }
}
