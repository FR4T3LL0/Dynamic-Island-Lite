//! Owns one top-edge reservation on the island's UI thread.
use island_core::{
    display::Monitor,
    error::{AppError, AppResult},
};
use std::cell::RefCell;
use tauri::AppHandle;
use windows::{
    core::w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        UI::{
            Shell::{
                DefSubclassProc, RemoveWindowSubclass, SHAppBarMessage, SetWindowSubclass, ABE_TOP,
                ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, ABN_POSCHANGED, APPBARDATA,
            },
            WindowsAndMessaging::{PostMessageW, RegisterWindowMessageW, WM_APP, WM_NCDESTROY},
        },
    },
};

const SUBCLASS: usize = 0x4449_4150;
const NOTIFY: u32 = WM_APP + 0x442;
const REFRESH: u32 = WM_APP + 0x443;

struct Reservation {
    app: AppHandle,
    hwnd: HWND,
    taskbar_created: u32,
    requested: RECT,
    height: i32,
    result: Option<RECT>,
    registered: bool,
}

thread_local! { static RESERVATION: RefCell<Option<Reservation>> = const { RefCell::new(None) }; }

fn data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: NOTIFY,
        uEdge: ABE_TOP,
        ..Default::default()
    }
}

fn rejected() -> AppError {
    AppError::new(
        "window_error",
        "Windows konnte den oberen Bildschirmrand nicht für die Insel reservieren.",
    )
}

fn unregister(state: &mut Reservation) {
    if state.registered {
        unsafe {
            SHAppBarMessage(ABM_REMOVE, &mut data(state.hwnd));
        }
        state.registered = false;
    }
}

pub fn release() -> bool {
    if let Some(mut state) = RESERVATION.with_borrow_mut(Option::take) {
        unregister(&mut state);
        unsafe {
            let _ = RemoveWindowSubclass(state.hwnd, Some(callback), SUBCLASS);
        }
        return true;
    }
    false
}

pub fn configure(
    app: &AppHandle,
    hwnd: HWND,
    monitor: &Monitor,
    logical_height: f64,
) -> AppResult<(i32, bool)> {
    let bounds = monitor.bounds;
    let height = (logical_height * monitor.scale).round() as i32;
    let requested = RECT {
        left: bounds.left,
        top: bounds.top,
        right: bounds.right,
        bottom: bounds.top + height,
    };
    let mut state = if let Some(state) = RESERVATION.with_borrow_mut(Option::take) {
        state
    } else {
        let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        if taskbar_created == 0
            || !unsafe { SetWindowSubclass(hwnd, Some(callback), SUBCLASS, 0) }.as_bool()
        {
            return Err(rejected());
        }
        Reservation {
            app: app.clone(),
            hwnd,
            taskbar_created,
            requested,
            height,
            result: None,
            registered: false,
        }
    };
    let update = (|| {
        if state.requested == requested && state.height == height {
            if let Some(result) = state.result {
                return Ok((result.top, false));
            }
        } else {
            // Remove the old monitor's reservation before requesting the new edge.
            unregister(&mut state);
        }
        state.requested = requested;
        state.height = height;
        state.result = None;
        let mut bar = data(hwnd);
        if !state.registered {
            state.registered = unsafe { SHAppBarMessage(ABM_NEW, &mut bar) } != 0;
            if !state.registered {
                return Err(rejected());
            }
        }
        bar.rc = requested;
        unsafe {
            SHAppBarMessage(ABM_QUERYPOS, &mut bar);
        }
        bar.rc.bottom = bar.rc.top + height;
        if unsafe { SHAppBarMessage(ABM_SETPOS, &mut bar) } == 0 {
            unregister(&mut state);
            return Err(rejected());
        }
        state.result = Some(bar.rc);
        Ok((bar.rc.top, true))
    })();
    RESERVATION.with_borrow_mut(|slot| *slot = Some(state));
    update
}

unsafe extern "system" fn callback(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    let _ = std::panic::catch_unwind(|| {
        if message == WM_NCDESTROY {
            release();
            return;
        }
        if message == REFRESH {
            let app = RESERVATION.with_borrow(|slot| slot.as_ref().map(|state| state.app.clone()));
            if let Some(app) = app {
                let _ = crate::shell::refresh_geometry(&app);
            }
            return;
        }
        let refresh = RESERVATION.with_borrow_mut(|slot| {
            let Some(state) = slot.as_mut() else {
                return false;
            };
            if message == state.taskbar_created {
                state.registered = false;
            } else if message != NOTIFY || wparam.0 != ABN_POSCHANGED as usize {
                return false;
            }
            state.result = None;
            true
        });
        if refresh {
            let _ = unsafe { PostMessageW(Some(hwnd), REFRESH, WPARAM(0), LPARAM(0)) };
        }
    });
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
