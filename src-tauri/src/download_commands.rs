//! Tauri side of the model downloads (commands and events). The download
//! logic itself (`downloads.rs`) has no Tauri dependency: the installer
//! (`src-installer`) reuses it as is.

use std::sync::Arc;

use crate::downloads::{DownloadEvents, DownloadStatus, Downloader, TranslationPlan};

pub const EVENT: &str = "models://download";

/// Progress to every window; once complete, the engines pick the files up.
pub struct TauriDownloadEvents(pub tauri::AppHandle);

impl DownloadEvents for TauriDownloadEvents {
    fn progress(&self, status: &DownloadStatus) {
        use tauri::Emitter;
        let _ = self.0.emit(EVENT, status);
    }
    fn finished(&self) {
        use tauri::Manager;
        if let Some(s) = self.0.try_state::<Arc<crate::speech::SpeechService>>() {
            s.rescan();
        }
        if let Some(t) = self
            .0
            .try_state::<Arc<crate::translation::TranslationService>>()
        {
            t.rescan();
        }
    }
}

#[tauri::command]
pub fn models_download_status(
    d: tauri::State<'_, Arc<Downloader>>,
    model: String,
    translation: bool,
    plan: Option<TranslationPlan>,
) -> DownloadStatus {
    let plan = translation.then(|| plan.unwrap_or_default());
    d.status(&crate::paths::models_dir(), &model, plan.as_ref())
}

/// Starts fetching the missing models (no-op if complete or already running).
#[tauri::command]
pub fn models_download_start(
    d: tauri::State<'_, Arc<Downloader>>,
    model: String,
    translation: bool,
    plan: Option<TranslationPlan>,
) -> DownloadStatus {
    let dir = crate::paths::models_dir();
    let plan = translation.then(|| plan.unwrap_or_default());
    d.start(dir.clone(), &model, plan.as_ref());
    d.status(&dir, &model, plan.as_ref())
}

#[tauri::command]
pub fn models_download_cancel(d: tauri::State<'_, Arc<Downloader>>) {
    d.cancel();
}
