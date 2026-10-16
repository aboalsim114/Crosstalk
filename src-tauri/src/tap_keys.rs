//! One-key shortcuts that `RegisterHotKey` cannot provide: a quick tap of the
//! left **Alt** opens Répondre, a quick tap of **Alt Gr** starts or stops the
//! dictation.
//!
//! A tap is the key pressed then released within [`MAX_TAP`] with no other key
//! pressed meanwhile: Alt+Tab, Alt Gr+E (€), Ctrl+Alt+F8… never trigger it.
//! Keys already held when the tap starts (W/A/S/D while moving in CS2) do not
//! cancel it; a modifier (Ctrl, Shift, Windows) held at any moment does.
//!
//! Ignored while Crosstalk itself holds CS2's voice key (when it is set to
//! Alt in Voix IA, its own press is not a tap).
//!
//! Read with `GetAsyncKeyState`, polled: no keyboard hook, no key recorded,
//! nothing touches the game process (the same call as voice/ptt.rs).

// Outside Windows only the tests use the detector.
#![cfg_attr(not(windows), allow(dead_code))]

use std::time::{Duration, Instant};

use crate::actions::Action;

/// A press longer than this is not a tap (the key is held for something else).
pub const MAX_TAP: Duration = Duration::from_millis(600);
/// Polling period while no tap key is down, and while one is.
const IDLE_POLL: Duration = Duration::from_millis(15);
const ARMED_POLL: Duration = Duration::from_millis(10);

// Windows virtual-key codes.
const VK_SHIFT: u8 = 0x10;
const VK_CONTROL: u8 = 0x11;
const VK_MENU: u8 = 0x12;
const VK_LWIN: u8 = 0x5B;
const VK_RWIN: u8 = 0x5C;
const VK_LSHIFT: u8 = 0xA0;
const VK_RSHIFT: u8 = 0xA1;
const VK_LCONTROL: u8 = 0xA2;
const VK_RCONTROL: u8 = 0xA3;
const VK_LMENU: u8 = 0xA4;
const VK_RMENU: u8 = 0xA5;

const MODIFIERS: [u8; 10] = [
    VK_SHIFT,
    VK_CONTROL,
    VK_LWIN,
    VK_RWIN,
    VK_LSHIFT,
    VK_RSHIFT,
    VK_LCONTROL,
    VK_RCONTROL,
    VK_LMENU,
    VK_RMENU,
];

/// A one-key shortcut.
#[derive(Debug, Clone, Copy)]
pub struct TapKey {
    pub vk: u8,
    pub action: Action,
    pub label: &'static str,
    /// Keys the tap key itself reports as down: the generic Alt, and the left
    /// Ctrl that Windows adds to Alt Gr on most European layouts.
    pub companions: &'static [u8],
}

pub const TAP_KEYS: [TapKey; 2] = [
    TapKey {
        vk: VK_LMENU,
        action: Action::Reply,
        label: "Alt",
        companions: &[VK_MENU],
    },
    TapKey {
        vk: VK_RMENU,
        action: Action::Dictate,
        label: "Alt Gr",
        companions: &[VK_MENU, VK_LCONTROL, VK_CONTROL],
    },
];

/// What happened on the other keys since the previous poll.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Others {
    /// A key that was up is now down.
    pub new_press: bool,
    /// A modifier (Ctrl, Shift, Windows, the other Alt) is down.
    pub modifier: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Armed(Instant),
    /// Not a tap any more: wait for the key to be released.
    Spoiled,
}

/// Tap detection for one key, fed at every poll.
#[derive(Debug)]
pub struct TapDetector {
    state: State,
}

impl Default for TapDetector {
    fn default() -> Self {
        Self { state: State::Idle }
    }
}

impl TapDetector {
    /// True once, when the key is released at the end of a tap.
    pub fn feed(&mut self, down: bool, others: Others, now: Instant) -> bool {
        match self.state {
            State::Idle if down => {
                self.state = if others.modifier || others.new_press {
                    State::Spoiled
                } else {
                    State::Armed(now)
                };
                false
            }
            State::Idle => false,
            State::Armed(start) => {
                if others.modifier || others.new_press || now - start > MAX_TAP {
                    self.state = if down { State::Spoiled } else { State::Idle };
                    false
                } else if down {
                    false
                } else {
                    self.state = State::Idle;
                    true
                }
            }
            State::Spoiled => {
                if !down {
                    self.state = State::Idle;
                }
                false
            }
        }
    }

    pub fn armed(&self) -> bool {
        matches!(self.state, State::Armed(_))
    }
}

/// The keys that are down, as a 256-bit set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeySet([u64; 4]);

impl KeySet {
    pub fn set(&mut self, vk: u8) {
        self.0[(vk / 64) as usize] |= 1 << (vk % 64);
    }

    pub fn has(&self, vk: u8) -> bool {
        self.0[(vk / 64) as usize] & (1 << (vk % 64)) != 0
    }

    /// The other keys seen by the detector of `key`, from the previous set.
    pub fn others(&self, prev: &KeySet, key: &TapKey) -> Others {
        let ignored = |vk: u8| vk == key.vk || key.companions.contains(&vk);
        let new_press = (1..=254u8).any(|vk| !ignored(vk) && self.has(vk) && !prev.has(vk));
        let modifier = MODIFIERS.iter().any(|&vk| !ignored(vk) && self.has(vk));
        Others {
            new_press,
            modifier,
        }
    }
}

/// Starts the polling thread (Windows). `false` when it could not start.
#[cfg(windows)]
pub fn start(app: tauri::AppHandle) -> bool {
    std::thread::Builder::new()
        .name("tap-keys".into())
        .spawn(move || run(&app))
        .is_ok()
}

#[cfg(not(windows))]
pub fn start(_app: tauri::AppHandle) -> bool {
    false
}

#[cfg(windows)]
fn run(app: &tauri::AppHandle) {
    use tauri::Manager;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    // SAFETY: plain Win32 call, no pointer.
    let down = |vk: u8| (unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000) != 0;
    let snapshot = || {
        let mut s = KeySet::default();
        for vk in 1..=254u8 {
            if down(vk) {
                s.set(vk);
            }
        }
        s
    };
    let mut detectors: Vec<TapDetector> = TAP_KEYS.iter().map(|_| TapDetector::default()).collect();
    let mut prev = KeySet::default();
    let mut scanning = false;
    loop {
        let any_tap_key = TAP_KEYS.iter().any(|k| down(k.vk));
        // The whole keyboard is read only around a tap (a few hundred ms):
        // otherwise only the two tap keys, which costs nothing.
        if any_tap_key || detectors.iter().any(TapDetector::armed) {
            let now_set = snapshot();
            if !scanning {
                // Keys held when the tap starts (moving in CS2) are not new presses.
                prev = now_set;
                scanning = true;
            }
            let now = Instant::now();
            let ours = app
                .try_state::<std::sync::Arc<crate::voice::VoiceService>>()
                .is_some_and(|v| v.holding_key());
            for (key, det) in TAP_KEYS.iter().zip(detectors.iter_mut()) {
                let mut others = now_set.others(&prev, key);
                others.modifier |= ours;
                if det.feed(now_set.has(key.vk), others, now) {
                    crate::actions::dispatch(app, key.action);
                }
            }
            prev = now_set;
            std::thread::sleep(ARMED_POLL);
        } else {
            scanning = false;
            for det in detectors.iter_mut() {
                det.feed(false, Others::default(), Instant::now());
            }
            std::thread::sleep(IDLE_POLL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Others = Others {
        new_press: false,
        modifier: false,
    };
    const PRESS: Others = Others {
        new_press: true,
        modifier: false,
    };
    const MOD: Others = Others {
        new_press: false,
        modifier: true,
    };

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn a_quick_tap_fires_once_on_release() {
        let t0 = Instant::now();
        let mut d = TapDetector::default();
        assert!(!d.feed(true, NONE, t0));
        assert!(!d.feed(true, NONE, ms(t0, 100)));
        assert!(d.feed(false, NONE, ms(t0, 150)));
        assert!(!d.feed(false, NONE, ms(t0, 200)));
    }

    #[test]
    fn another_key_during_the_press_cancels_it() {
        // Alt+Tab, Alt Gr+E (€): no shortcut.
        let t0 = Instant::now();
        let mut d = TapDetector::default();
        d.feed(true, NONE, t0);
        assert!(!d.feed(true, PRESS, ms(t0, 50)));
        assert!(!d.feed(false, NONE, ms(t0, 100)));
        // The next tap works again.
        d.feed(true, NONE, ms(t0, 500));
        assert!(d.feed(false, NONE, ms(t0, 550)));
    }

    #[test]
    fn a_held_modifier_or_a_long_press_is_not_a_tap() {
        let t0 = Instant::now();
        let mut d = TapDetector::default();
        // Ctrl held, then Alt tapped (Ctrl+Alt+F8 being typed).
        assert!(!d.feed(true, MOD, t0));
        assert!(!d.feed(false, NONE, ms(t0, 80)));
        // Held for a second.
        d.feed(true, NONE, ms(t0, 1000));
        d.feed(true, NONE, ms(t0, 1700));
        assert!(!d.feed(false, NONE, ms(t0, 2000)));
    }

    #[test]
    fn keys_held_before_the_tap_do_not_cancel_it() {
        const W: u8 = 0x57;
        const TAB: u8 = 0x09;
        let alt = &TAP_KEYS[0];
        let mut before = KeySet::default();
        before.set(W);
        let mut during = before;
        during.set(alt.vk);
        during.set(VK_MENU);
        assert_eq!(during.others(&before, alt), NONE);
        let mut tab = during;
        tab.set(TAB);
        assert!(tab.others(&during, alt).new_press);
    }

    #[test]
    fn alt_gr_ignores_the_ctrl_windows_adds_to_it_but_not_a_real_ctrl() {
        let alt_gr = &TAP_KEYS[1];
        let mut s = KeySet::default();
        for vk in [VK_RMENU, VK_MENU, VK_LCONTROL, VK_CONTROL] {
            s.set(vk);
        }
        assert_eq!(s.others(&KeySet::default(), alt_gr), NONE);
        // For the left Alt, that left Ctrl is a real modifier.
        assert!(s.others(&KeySet::default(), &TAP_KEYS[0]).modifier);
        let mut right_ctrl = s;
        right_ctrl.set(VK_RCONTROL);
        assert!(right_ctrl.others(&s, alt_gr).modifier);
    }

    #[test]
    fn alt_opens_reply_and_alt_gr_dictates() {
        assert_eq!(TAP_KEYS[0].action, Action::Reply);
        assert_eq!(TAP_KEYS[1].action, Action::Dictate);
    }
}
