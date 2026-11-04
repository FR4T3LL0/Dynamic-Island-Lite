use crate::state::AppState;
use island_core::{
    display::{covers_island, island_rect, select, Monitor},
    error::{AppError, AppResult},
    settings::Settings,
};
use island_win::display_events::{DisplayEvents, Observation};
use serde::Serialize;
use std::sync::{atomic::Ordering, Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayState {
    pub monitors: Vec<Monitor>,
    pub selected_id: Option<String>,
    pub fallback: bool,
    pub fullscreen: bool,
    pub smart_covered: bool,
    pub error: Option<AppError>,
}
#[derive(Default)]
pub struct DisplayService {
    observation: Mutex<Option<Observation>>,
    worker: Mutex<Option<DisplayEvents>>,
}
impl DisplayService {
    pub fn start(&self, app: AppHandle, filter: Option<u32>) -> AppResult<()> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| AppError::new("conflict", "Die Bildschirmüberwachung ist gesperrt."))?;
        if worker.is_some() {
            return Ok(());
        }
        *worker = Some(DisplayEvents::start(
            filter,
            Arc::new(move |observation, refresh| {
                let state = app.state::<AppState>();
                let (layout_changed, changed) = {
                    let settings = state.settings.lock().ok();
                    state
                        .display
                        .observation
                        .lock()
                        .map(|mut current| {
                            let changed = refresh
                                || current
                                    .as_ref()
                                    .is_none_or(|old| old.monitors != observation.monitors);
                            let visibility_changed = current.as_ref().is_none_or(|old| {
                                old.fullscreen_monitors != observation.fullscreen_monitors
                                    || old.error != observation.error
                                    || settings.as_deref().is_some_and(|settings| {
                                        smart_covered(old, settings)
                                            != smart_covered(&observation, settings)
                                    })
                            });
                            *current = Some(observation);
                            (changed, changed || visibility_changed)
                        })
                        .unwrap_or_default()
                };
                if changed {
                    if let Err(error) = publish(&app) {
                        let _ = app.emit("service-error", error);
                    }
                }
                if layout_changed {
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Err(error) = crate::shell::refresh_geometry(&handle) {
                            let _ = handle.emit("service-error", error);
                        }
                    });
                }
            }),
        )?);
        Ok(())
    }
    pub fn snapshot(&self, settings: &Settings) -> AppResult<DisplayState> {
        let observation = self
            .observation
            .lock()
            .map_err(|_| AppError::new("conflict", "Die Bildschirmdaten sind gesperrt."))?;
        let Some(observation) = observation.as_ref() else {
            return Ok(DisplayState {
                monitors: vec![],
                selected_id: None,
                fallback: false,
                fullscreen: false,
                smart_covered: false,
                error: None,
            });
        };
        let (monitor, fallback) = select(&observation.monitors, settings.monitor.as_deref())?;
        Ok(DisplayState {
            monitors: observation.monitors.clone(),
            selected_id: Some(monitor.id.clone()),
            fallback,
            fullscreen: observation.fullscreen_monitors.contains(&monitor.id),
            smart_covered: smart_covered(observation, settings),
            error: observation.error.clone(),
        })
    }
    pub fn refresh(&self) -> AppResult<()> {
        self.worker
            .lock()
            .map_err(|_| AppError::new("conflict", "Die Bildschirmüberwachung ist gesperrt."))?
            .as_ref()
            .ok_or_else(|| AppError::new("window_error", "Die Bildschirmüberwachung läuft nicht."))?
            .refresh()
    }
    pub fn at_peek_edge(&self, requested: Option<&str>, width: f64, x: i32, y: i32) -> bool {
        let Ok(observation) = self.observation.lock() else {
            return false;
        };
        let Some(observation) = observation.as_ref() else {
            return false;
        };
        let Ok((monitor, _)) = select(&observation.monitors, requested) else {
            return false;
        };
        let top = monitor.bounds.top;
        let band = (3.0 * monitor.scale).ceil() as i32;
        if y < top
            || y >= top.saturating_add(band)
            || observation.fullscreen_monitors.contains(&monitor.id)
        {
            return false;
        }
        island_core::display::island_rect(monitor, width, 36.0, 0.0)
            .is_ok_and(|rect| x >= rect.left && x < rect.right)
    }
    pub fn stop(&self) {
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(mut events) = worker.take() {
                let _ = events.stop();
            }
        }
    }
}
fn smart_covered(observation: &Observation, settings: &Settings) -> bool {
    if !settings.smart {
        return false;
    }
    let Some(window) = observation.foreground else {
        return false;
    };
    let Ok((monitor, _)) = select(&observation.monitors, settings.monitor.as_deref()) else {
        return false;
    };
    island_rect(
        monitor,
        settings.compact_width(),
        36.0,
        settings.island_gap(),
    )
    .is_ok_and(|rect| covers_island(window, monitor, rect))
}
#[tauri::command]
pub fn refresh_displays(app: AppHandle) -> AppResult<()> {
    app.state::<AppState>().display.refresh()
}
pub fn publish(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let (display, revision) = {
        // Reserve the revision while the selection is still locked. Otherwise
        // an old selection could be published after a newer settings commit.
        let settings = state
            .settings
            .lock()
            .map_err(|_| AppError::new("conflict", "Die Einstellungen sind gesperrt."))?;
        let display = state.display.snapshot(&settings)?;
        let revision = state.revision.fetch_add(1, Ordering::AcqRel) + 1;
        (display, revision)
    };
    app.emit(
        "display-updated",
        serde_json::json!({"display":display,"revision":revision}),
    )
    .map_err(|e| AppError::new("window_error", e.to_string()))
}
