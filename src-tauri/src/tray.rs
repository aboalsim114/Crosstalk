//! Notification-area (system tray) icon: Crosstalk keeps running there when the
//! main window is closed or minimised.

use std::collections::HashMap;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::actions::{self, Action};

const TRAY_ID: &str = "crosstalk";

/// The menu items, kept to translate them when the interface language
/// changes (French until the interface sends its labels).
pub struct TrayItems(Vec<(&'static str, MenuItem<Wry>)>);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Afficher Crosstalk", true, None::<&str>)?;
    let session = MenuItem::with_id(
        app,
        "session",
        "Démarrer / arrêter la session",
        true,
        None::<&str>,
    )?;
    let overlay = MenuItem::with_id(app, "overlay", "Overlay\tCtrl+Alt+F8", true, None::<&str>)?;
    let reply = MenuItem::with_id(
        app,
        "reply",
        "Répondre dans leur langue\tAlt",
        true,
        None::<&str>,
    )?;
    let dictate = MenuItem::with_id(
        app,
        "dictate",
        "Dicter une réponse\tAlt Gr",
        true,
        None::<&str>,
    )?;
    let pause = MenuItem::with_id(app, "pause", "Pause\tCtrl+Alt+F9", true, None::<&str>)?;
    let emergency = MenuItem::with_id(
        app,
        "emergency",
        "Arrêt d’urgence\tCtrl+Alt+F10",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quitter Crosstalk", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &sep()?,
            &session,
            &overlay,
            &reply,
            &dictate,
            &pause,
            &emergency,
            &sep()?,
            &quit,
        ],
    )?;

    app.manage(TrayItems(vec![
        ("show", show.clone()),
        ("session", session.clone()),
        ("overlay", overlay.clone()),
        ("reply", reply.clone()),
        ("dictate", dictate.clone()),
        ("pause", pause.clone()),
        ("emergency", emergency.clone()),
        ("quit", quit.clone()),
    ]));

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Crosstalk — traduction CS2")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => actions::show_main(app),
            "session" => actions::dispatch(app, Action::Session),
            "overlay" => actions::dispatch(app, Action::Overlay),
            "pause" => actions::dispatch(app, Action::Pause),
            "reply" => actions::dispatch(app, Action::Reply),
            "dictate" => actions::dispatch(app, Action::Dictate),
            "emergency" => actions::dispatch(app, Action::Emergency),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                actions::show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Labels of the tray menu in the interface language, by item id; `tooltip`
/// for the icon's tooltip. Unknown ids and empty or overlong labels are
/// ignored.
#[tauri::command]
pub fn ui_set_tray_labels(app: AppHandle, labels: HashMap<String, String>) -> Result<(), String> {
    let ok = |s: &&String| valid_label(s);
    if let Some(items) = app.try_state::<TrayItems>() {
        for (id, item) in &items.0 {
            if let Some(text) = labels.get(*id).filter(ok) {
                item.set_text(text).map_err(|e| e.to_string())?;
            }
        }
    }
    if let (Some(tray), Some(tip)) = (app.tray_by_id(TRAY_ID), labels.get("tooltip").filter(ok)) {
        tray.set_tooltip(Some(tip)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A menu label from the interface: not blank, not absurdly long.
fn valid_label(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 80
}

#[cfg(test)]
mod tests {
    #[test]
    fn labels_from_the_interface_are_checked() {
        assert!(super::valid_label("Quit Crosstalk"));
        assert!(super::valid_label("Аварийная остановка\tCtrl+Alt+F10"));
        assert!(!super::valid_label("   "));
        assert!(!super::valid_label(&"x".repeat(81)));
    }
}
