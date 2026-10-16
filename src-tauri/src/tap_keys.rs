//! Shortcuts on the mouse's side buttons, which `RegisterHotKey` cannot
//! provide: **Mouse 4** opens Répondre, **Mouse 5** starts or stops the
//! dictation.
//!
//! A click counts when the button is released within [`MAX_TAP`] with no
//! other key or button pressed meanwhile. Keys already held when the click
//! starts do not cancel it: moving (W/A/S/D), walking (Shift) or crouching
//! (Ctrl) in CS2 while clicking still works.
//!
//! Ignored while Crosstalk itself holds CS2's voice key (when it is set to
//! Mouse4 or Mouse5 in Voix IA, its own press is not a click).
//!
//! Read with `GetAsyncKeyState`, polled: no keyboard or mouse hook, no key
//! recorded, nothing touches the game process (the same call as voice/ptt.rs).
//! CS2 still receives the click.

// Outside Windows only the tests use the detector.
#![cfg_attr(not(windows), allow(dead_code))]

use std::time::{Duration, Instant};

use crate::actions::Action;

/// A press longer than this is not a click (the button is held for something else).
pub const MAX_TAP: Duration = Duration::from_millis(600);
/// Polling period while no shortcut button is down, and while one is.
const IDLE_POLL: Duration = Duration::from_millis(15);
const ARMED_POLL: Duration = Duration::from_millis(10);

// Windows virtual-key codes of the side buttons.
const VK_XBUTTON1: u8 = 0x05;
const VK_XBUTTON2: u8 = 0x06;

/// A one-button shortcut.
#[derive(Debug, Clone, Copy)]
pub struct TapKey {
    pub vk: u8,
    pub action: Action,
    pub label: &'static str,
}

pub const TAP_KEYS: [TapKey; 2] = [
    TapKey {
        vk: VK_XBUTTON1,
        action: Action::Reply,
        label: "Mouse 4",
    },
    TapKey {
        vk: VK_XBUTTON2,
        action: Action::Dictate,
        label: "Mouse 5",
    },
];

/// What happened elsewhere since the previous poll.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Others {
    /// A key or button that was up is now down.
    pub new_press: bool,
    /// Not the user's click (Crosstalk holds the voice key).
    pub blocked: bool,
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
                self.state = if others.blocked || others.new_press {
                    State::Spoiled
                } else {
                    State::Armed(now)
                };
                false
            }
            State::Idle => false,
            State::Armed(start) => {
                if others.blocked || others.new_press || now - start > MAX_TAP {
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
        let new_press = (1..=254u8).any(|vk| vk != key.vk && self.has(vk) && !prev.has(vk));
        Others {
            new_press,
            blocked: false,
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
        // The whole keyboard is read only around a click (a few hundred ms):
        // otherwise only the two side buttons, which costs nothing.
        if any_tap_key || detectors.iter().any(TapDetector::armed) {
            let now_set = snapshot();
            if !scanning {
                // Keys held when the click starts (moving in CS2) are not new presses.
                prev = now_set;
                scanning = true;
            }
            let now = Instant::now();
            let ours = app
                .try_state::<std::sync::Arc<crate::voice::VoiceService>>()
                .is_some_and(|v| v.holding_key());
            for (key, det) in TAP_KEYS.iter().zip(detectors.iter_mut()) {
                let mut others = now_set.others(&prev, key);
                others.blocked |= ours;
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
        blocked: false,
    };
    const PRESS: Others = Others {
        new_press: true,
        blocked: false,
    };
    const OURS: Others = Others {
        new_press: false,
        blocked: true,
    };

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn a_click_fires_once_on_release() {
        let t0 = Instant::now();
        let mut d = TapDetector::default();
        assert!(!d.feed(true, NONE, t0));
        assert!(!d.feed(true, NONE, ms(t0, 100)));
        assert!(d.feed(false, NONE, ms(t0, 150)));
        assert!(!d.feed(false, NONE, ms(t0, 200)));
    }

    #[test]
    fn another_key_during_the_click_cancels_it() {
        let t0 = Instant::now();
        let mut d = TapDetector::default();
        d.feed(true, NONE, t0);
        assert!(!d.feed(true, PRESS, ms(t0, 50)));
        assert!(!d.feed(false, NONE, ms(t0, 100)));
        // The next click works again.
        d.feed(true, NONE, ms(t0, 500));
        assert!(d.feed(false, NONE, ms(t0, 550)));
    }

    #[test]
    fn a_long_press_or_crosstalks_own_press_is_not_a_click() {
        let t0 = Instant::now();
        let mut d = TapDetector::default();
        d.feed(true, NONE, t0);
        d.feed(true, NONE, ms(t0, 700));
        assert!(!d.feed(false, NONE, ms(t0, 900)));
        // The voice key held by Crosstalk (set to Mouse4 in Voix IA).
        assert!(!d.feed(true, OURS, ms(t0, 1000)));
        assert!(!d.feed(false, NONE, ms(t0, 1100)));
    }

    #[test]
    fn keys_held_before_the_click_do_not_cancel_it() {
        // Moving forward, walking (Shift) and crouching (Ctrl) in CS2.
        const W: u8 = 0x57;
        const LSHIFT: u8 = 0xA0;
        const LCTRL: u8 = 0xA2;
        const R: u8 = 0x52;
        let m4 = &TAP_KEYS[0];
        let mut before = KeySet::default();
        for vk in [W, LSHIFT, LCTRL] {
            before.set(vk);
        }
        let mut during = before;
        during.set(m4.vk);
        assert_eq!(during.others(&before, m4), NONE);
        // Reloading (R) during the click: not the shortcut.
        let mut reload = during;
        reload.set(R);
        assert!(reload.others(&during, m4).new_press);
    }

    #[test]
    fn mouse4_opens_reply_and_mouse5_dictates() {
        assert_eq!((TAP_KEYS[0].vk, TAP_KEYS[0].action), (0x05, Action::Reply));
        assert_eq!(
            (TAP_KEYS[1].vk, TAP_KEYS[1].action),
            (0x06, Action::Dictate)
        );
    }
}
