//! In-game subtitle overlay: a separate transparent, always-on-top, frameless window
//! that does not appear in the taskbar and, by default, lets clicks pass through to
//! the game (CS2 must run in borderless windowed mode; exclusive fullscreen is not
//! supported without injection, which Crosstalk never does).

use std::sync::Mutex;

use serde::Deserialize;
use tauri::{
    AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

/// Corner the overlay was last anchored to (it follows CS2 to its screen).
static CORNER: Mutex<Corner> = Mutex::new(Corner::Tr);

pub const LABEL: &str = "overlay";
/// Logical size of the overlay window; the subtitle stack is 380 px wide.
pub const WIDTH: f64 = 440.0;
pub const HEIGHT: f64 = 560.0;
/// Distance from the screen edge, in logical pixels.
pub const MARGIN: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Corner {
    Tl,
    Tr,
    Bl,
    Br,
}

/// Top-left position of a `win` sized window anchored in `corner` of a work area.
pub fn corner_position(
    corner: Corner,
    area_pos: (i32, i32),
    area_size: (u32, u32),
    win: (u32, u32),
    margin: i32,
) -> (i32, i32) {
    let (ax, ay) = area_pos;
    let (aw, ah) = (area_size.0 as i32, area_size.1 as i32);
    let (ww, wh) = (win.0 as i32, win.1 as i32);
    let left = ax + margin;
    let right = ax + aw - ww - margin;
    let top = ay + margin;
    let bottom = ay + ah - wh - margin;
    match corner {
        Corner::Tl => (left, top),
        Corner::Tr => (right, top),
        Corner::Bl => (left, bottom),
        Corner::Br => (right, bottom),
    }
}

/// Creates the (hidden) overlay window at startup.
pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Crosstalk — overlay")
        .inner_size(WIDTH, HEIGHT)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;
    // Click-through is applied when the window is shown (see `overlay_configure`):
    // some platforms have no native surface for a window that was never shown.
    place(&win, Corner::Tr)?;
    Ok(win)
}

/// Top-left corner of the screen showing CS2's window (found by its title;
/// `FindWindowW` + `MonitorFromWindow` only read the window's position, the
/// game's process is never opened).
#[cfg(windows)]
pub fn cs2_screen_origin() -> Option<(i32, i32)> {
    use windows::core::w;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL,
    };
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    // SAFETY: plain Win32 queries with valid, initialized arguments.
    unsafe {
        let hwnd = FindWindowW(None, w!("Counter-Strike 2")).ok()?;
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL);
        if mon.is_invalid() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(mon, &mut info).ok().ok()?;
        Some((info.rcMonitor.left, info.rcMonitor.top))
    }
}

#[cfg(not(windows))]
pub fn cs2_screen_origin() -> Option<(i32, i32)> {
    None
}

/// The screen CS2 is on, else the main screen.
fn target_monitor(win: &WebviewWindow) -> tauri::Result<Option<Monitor>> {
    if let Some(origin) = cs2_screen_origin() {
        let found = win
            .available_monitors()?
            .into_iter()
            .find(|m| (m.position().x, m.position().y) == origin);
        if found.is_some() {
            return Ok(found);
        }
    }
    win.primary_monitor()
}

/// Keeps the visible overlay on CS2's screen (two-screen set-ups, or CS2 moved
/// to another screen). A position dragged on the same screen is kept.
pub fn follow_cs2(app: &AppHandle) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    if !win.is_visible().unwrap_or(false) || cs2_screen_origin().is_none() {
        return;
    }
    let (Ok(Some(target)), Ok(Some(current))) = (target_monitor(&win), win.current_monitor())
    else {
        return;
    };
    if target.position() != current.position() {
        let corner = *CORNER.lock().unwrap();
        let _ = place(&win, corner);
    }
}

fn place(win: &WebviewWindow, corner: Corner) -> tauri::Result<()> {
    *CORNER.lock().unwrap() = corner;
    let Some(monitor) = target_monitor(win)? else {
        return Ok(());
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let size = LogicalSize::new(WIDTH, HEIGHT).to_physical::<u32>(scale);
    let (x, y) = corner_position(
        corner,
        (area.position.x, area.position.y),
        (area.size.width, area.size.height),
        (size.width, size.height),
        (MARGIN * scale).round() as i32,
    );
    win.set_position(PhysicalPosition::new(x, y))
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayConfig {
    pub visible: bool,
    /// Clicks go through to the game. Turned off while repositioning.
    pub click_through: bool,
    /// Snap to this corner; `None` keeps a position the user dragged to.
    pub corner: Option<Corner>,
}

#[tauri::command]
pub fn overlay_configure(app: AppHandle, config: OverlayConfig) -> Result<(), String> {
    let win = app
        .get_webview_window(LABEL)
        .ok_or("overlay window missing")?;
    let apply = || -> tauri::Result<()> {
        if let Some(corner) = config.corner {
            place(&win, corner)?;
        }
        if config.visible {
            win.show()?;
            win.set_ignore_cursor_events(config.click_through)?;
            win.set_always_on_top(true)?;
        } else {
            win.hide()?;
        }
        Ok(())
    };
    apply().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: ((i32, i32), (u32, u32)) = ((0, 0), (1920, 1040));

    #[test]
    fn anchors_to_each_corner_with_margin() {
        let at = |c| corner_position(c, AREA.0, AREA.1, (440, 560), 24);
        assert_eq!(at(Corner::Tl), (24, 24));
        assert_eq!(at(Corner::Tr), (1920 - 440 - 24, 24));
        assert_eq!(at(Corner::Bl), (24, 1040 - 560 - 24));
        assert_eq!(at(Corner::Br), (1456, 456));
    }

    #[test]
    fn respects_offset_work_areas_on_secondary_monitors() {
        assert_eq!(
            corner_position(Corner::Tl, (-2560, 40), (2560, 1400), (440, 560), 24),
            (-2536, 64)
        );
    }

    #[test]
    fn parses_corner_names_from_the_ui() {
        let c: OverlayConfig =
            serde_json::from_str(r#"{"visible":true,"clickThrough":false,"corner":"bl"}"#).unwrap();
        assert_eq!(c.corner, Some(Corner::Bl));
        assert!(!c.click_through);
    }
}
