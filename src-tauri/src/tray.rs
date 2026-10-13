//! Notification-area (system tray) icon: Crosstalk keeps running there when the
//! main window is closed or minimised.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::actions::{self, Action};

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
        "Répondre dans leur langue\tCtrl+Alt+F7",
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
            &pause,
            &emergency,
            &sep()?,
            &quit,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id("crosstalk")
        .tooltip("Crosstalk — traduction CS2")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => actions::show_main(app),
            "session" => actions::dispatch(app, Action::Session),
            "overlay" => actions::dispatch(app, Action::Overlay),
            "pause" => actions::dispatch(app, Action::Pause),
            "reply" => actions::dispatch(app, Action::Reply),
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
