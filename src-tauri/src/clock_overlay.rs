use island_core::{
    display::Rect,
    error::{AppError, AppResult},
};
use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{webview::PageLoadEvent, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW,
};

const LABEL: &str = "clock-overlay";
static CREATING: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
thread_local! {
    static TARGET: Cell<Option<Rect>> = const { Cell::new(None) };
    static POSITIONED: Cell<Option<Rect>> = const { Cell::new(None) };
}

pub fn spring(island: &WebviewWindow) {
    if let Some(overlay) = island.get_webview_window(LABEL) {
        let _ = overlay.eval(
            r#"(() => {
            const digits = document.querySelector('.clock-overlay-text');
            if (digits) {
                digits.getAnimations().forEach(animation => animation.cancel());
                digits.animate([
                    {transform:'translateY(0) scale(1)'},
                    {transform:'translateY(2px) scale(1.07)',offset:.43},
                    {transform:'translateY(-.5px) scale(.985)',offset:.72},
                    {transform:'translateY(0) scale(1)'}
                ], {duration: 190, easing:'ease-out'});
            }
            })()"#,
        );
    }
}

pub fn mark_visibility(island: &WebviewWindow, action: &str) {
    let script = match action {
        "exit" => "document.documentElement.dataset.smartVisibility='exit'",
        "enter" => "document.documentElement.dataset.smartVisibility='enter'",
        _ => "delete document.documentElement.dataset.smartVisibility",
    };
    let _ = island.eval(script);
    if let Some(overlay) = island.get_webview_window(LABEL) {
        let _ = overlay.eval(script);
    }
}

fn error(error: tauri::Error) -> AppError {
    AppError::new("window_error", error.to_string())
}

// The target comes from the shell's selected monitor, never from its
// intermediate animation frames.
pub fn update(island: &WebviewWindow, target: Option<Rect>) -> AppResult<()> {
    TARGET.set(target);
    if READY.load(Ordering::Acquire) {
        if let Some(overlay) = island.get_webview_window(LABEL) {
            return apply(island, &overlay);
        }
    }
    if target.is_some()
        && island.get_webview_window(LABEL).is_none()
        && !CREATING.swap(true, Ordering::AcqRel)
    {
        let island = island.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = create(&island);
            CREATING.store(false, Ordering::Release);
        });
    }
    Ok(())
}

fn create(island: &WebviewWindow) -> AppResult<()> {
    let owner = island.clone();
    let mut builder = WebviewWindowBuilder::new(
        island.app_handle(),
        LABEL,
        WebviewUrl::App("index.html#clock-overlay".into()),
    )
    .owner(island)
    .map_err(error)?
    .title("Dynamic Island Lite – Uhr")
    .inner_size(56.0, 34.0)
    .visible(false)
    .focused(false)
    .focusable(false)
    .transparent(true)
    .decorations(false)
    .skip_taskbar(true)
    .shadow(false)
    .resizable(false)
    .always_on_top(true)
    .disable_drag_drop_handler()
    .on_page_load(move |overlay, payload| {
        if payload.event() != PageLoadEvent::Finished {
            return;
        }
        let island = owner.clone();
        let _ = owner.run_on_main_thread(move || {
            if overlay.set_ignore_cursor_events(true).is_ok() {
                READY.store(true, Ordering::Release);
                let _ = apply(&island, &overlay);
            }
        });
    });
    if let Some(root) = crate::diagnostic_root() {
        builder = builder.data_directory(root.join("webview"));
    }
    builder.build().map_err(error)?;
    Ok(())
}

fn apply(island: &WebviewWindow, overlay: &WebviewWindow) -> AppResult<()> {
    let visible = overlay.is_visible().map_err(error)?;
    let hwnd = overlay.hwnd().map_err(error)?;
    let window_error =
        |failure: windows::core::Error| AppError::new("window_error", failure.to_string());
    let flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER;
    let Some(bounds) = TARGET.get() else {
        if visible {
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    flags | SWP_NOMOVE | SWP_NOSIZE | SWP_HIDEWINDOW,
                )
            }
            .map_err(window_error)?;
        }
        if POSITIONED.take().is_some() {
            island
                .eval("delete document.documentElement.dataset.clockOverlay")
                .map_err(error)?;
        }
        return Ok(());
    };
    if POSITIONED.get() != Some(bounds) || !visible {
        // Show and place together without transferring focus from the owner.
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                flags | SWP_SHOWWINDOW,
            )
        }
        .map_err(window_error)?;
    }
    if POSITIONED.replace(Some(bounds)).is_none() {
        if let Err(failure) = island.eval("document.documentElement.dataset.clockOverlay='true'") {
            POSITIONED.set(None);
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    flags | SWP_NOMOVE | SWP_NOSIZE | SWP_HIDEWINDOW,
                )
            };
            return Err(error(failure));
        }
    }
    Ok(())
}
