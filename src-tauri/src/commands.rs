use crate::state::AppState;
use island_core::{
    error::{AppError, AppResult},
    timer::{TimerMode, TimerState},
};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub async fn set_volume(app: AppHandle, id: String, volume: f32) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let result = state.audio.set_volume(id, volume);
        if crate::diagnostic_root().is_some() {
            eprintln!(
                "audio-action-test: {}",
                serde_json::json!({"action":"volume","volume":volume,"ok":result.is_ok()})
            );
        }
        result
    })
    .await
    .map_err(|e| AppError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn timer_configure(
    app: AppHandle,
    mode: TimerMode,
    seconds: u64,
) -> AppResult<TimerState> {
    change_timer(app, move |timer| timer.configure(mode, seconds)).await
}
#[tauri::command]
pub async fn timer_start(app: AppHandle) -> AppResult<TimerState> {
    change_timer(app, |timer| timer.start()).await
}
#[tauri::command]
pub async fn timer_pause(app: AppHandle) -> AppResult<TimerState> {
    change_timer(app, |timer| timer.pause()).await
}
#[tauri::command]
pub async fn timer_reset(app: AppHandle) -> AppResult<TimerState> {
    change_timer(app, |timer| timer.reset()).await
}
async fn change_timer(
    app: AppHandle,
    change: impl FnOnce(&crate::timer_service::TimerService) -> AppResult<TimerState> + Send + 'static,
) -> AppResult<TimerState> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _transaction = state
            .settings_changes
            .lock()
            .map_err(|_| AppError::new("conflict", "Die Einstellungen werden gerade geändert."))?;
        change(&state.timer)
    })
    .await
    .map_err(|error| AppError::new("io_error", error.to_string()))?
}
