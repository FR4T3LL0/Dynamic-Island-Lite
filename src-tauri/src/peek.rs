use crate::state::AppState;
use island_core::error::{AppError, AppResult};
use std::{
    cell::RefCell,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, WebviewWindow};
use windows::Win32::{
    Foundation::{HWND, POINT},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
        WindowsAndMessaging::{GetPhysicalCursorPos, KillTimer, SetTimer},
    },
};

const TIMER: usize = 0x4449_504b;
const SAMPLE_MS: u32 = 75;

struct EdgeWatch {
    window: WebviewWindow,
    entered: Option<Instant>,
    fired: bool,
}

thread_local! {
    static WATCH: RefCell<Option<EdgeWatch>> = const { RefCell::new(None) };
}

// The island owns this UI-thread timer even while its HWND is hidden.
pub fn configure(window: &WebviewWindow, enabled: bool) -> AppResult<()> {
    let hwnd = window
        .hwnd()
        .map_err(|error| AppError::new("window_error", error.to_string()))?;
    WATCH.with_borrow_mut(|watch| {
        if enabled && watch.is_none() {
            if unsafe { SetTimer(Some(hwnd), TIMER, SAMPLE_MS, Some(tick)) } == 0 {
                return Err(AppError::new(
                    "window_error",
                    "Die Überwachung des Bildschirmrands konnte nicht gestartet werden.",
                ));
            }
            *watch = Some(EdgeWatch {
                window: window.clone(),
                entered: None,
                fired: false,
            });
        } else if !enabled && watch.take().is_some() {
            let _ = unsafe { KillTimer(Some(hwnd), TIMER) };
        }
        Ok(())
    })
}

fn edge_delay(window: &WebviewWindow) -> Option<Duration> {
    let state = window.state::<AppState>();
    if state.shutdown_in_progress.load(Ordering::Acquire)
        || [VK_LBUTTON, VK_MBUTTON, VK_RBUTTON]
            .iter()
            .any(|button| unsafe { GetAsyncKeyState(button.0 as i32) } < 0)
    {
        return None;
    }
    let mut point = POINT::default();
    unsafe { GetPhysicalCursorPos(&mut point) }.ok()?;
    let settings = state.settings.lock().ok()?;
    ((settings.peek || settings.smart)
        && state.display.at_peek_edge(
            settings.monitor.as_deref(),
            settings.compact_width(),
            point.x,
            point.y,
        ))
    .then(|| Duration::from_millis(u64::from(settings.edge_delay_ms)))
}

unsafe extern "system" fn tick(hwnd: HWND, _: u32, id: usize, _: u32) {
    if id != TIMER {
        return;
    }
    let result = std::panic::catch_unwind(|| {
        let ready = WATCH.with_borrow_mut(|watch| {
            let watch = watch.as_mut()?;
            let Some(dwell) = edge_delay(&watch.window) else {
                watch.entered = None;
                watch.fired = false;
                return None;
            };
            let entered = watch.entered.get_or_insert_with(Instant::now);
            if watch.fired || entered.elapsed() < dwell {
                return None;
            }
            // Staying at the edge produces a single reveal, until the pointer leaves.
            watch.fired = true;
            Some(watch.window.clone())
        });
        if let Some(window) = ready {
            let _ = window.emit("shell-peek", ());
        }
    });
    if result.is_err() {
        let _ = unsafe { KillTimer(Some(hwnd), TIMER) };
        WATCH.with_borrow_mut(|watch| *watch = None);
    }
}
