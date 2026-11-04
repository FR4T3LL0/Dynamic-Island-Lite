use crate::state::AppState;
use island_core::{
    error::{AppError, AppResult},
    settings::Appearance,
};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
struct ChangingShell<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for ChangingShell<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

fn map_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("io_error", error.to_string())
}

#[derive(Clone)]
pub struct ShellTarget {
    mode: String,
    module: Option<String>,
    generation: u64,
}

#[tauri::command]
pub async fn set_shell(app: AppHandle, mode: String, module: Option<String>) -> AppResult<()> {
    on_ui(app, move |handle| {
        update_shell(handle, &mode, module.as_deref(), true, true)
    })
    .await
}

pub fn refresh_geometry(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let target = state
        .shell_target
        .lock()
        .map_err(|_| AppError::new("conflict", "Der Inselzustand ist gesperrt."))?
        .clone();
    if let Some(target) = target {
        update_shell(app, &target.mode, target.module.as_deref(), false, false)?;
    }
    Ok(())
}

pub(crate) async fn on_ui<T: Send + 'static>(
    app: AppHandle,
    action: impl FnOnce(&AppHandle) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(action(&handle));
    })
    .map_err(map_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        receiver.recv_timeout(std::time::Duration::from_secs(5))
    })
    .await
    .map_err(map_error)?
    .map_err(map_error)?
}

#[tauri::command]
pub async fn shell_motion_probe(app: AppHandle, reset: bool) -> AppResult<serde_json::Value> {
    if crate::diagnostic_root().is_none() {
        return Err(AppError::new(
            "unsupported",
            "Diese Testaktion ist nicht verfügbar.",
        ));
    }
    on_ui(app, move |handle| {
        let window = handle
            .get_webview_window("island")
            .ok_or_else(|| AppError::new("not_found", "Die Insel fehlt."))?;
        crate::shell_motion::probe(reset, window.hwnd().map_err(map_error)?)
    })
    .await
}

fn update_shell(
    app: &AppHandle,
    mode: &str,
    module: Option<&str>,
    activate: bool,
    animate: bool,
) -> AppResult<()> {
    let window = app
        .get_webview_window("island")
        .ok_or_else(|| AppError::new("not_found", "Die Insel ist nicht verfügbar."))?;
    if ![
        "compact",
        "compact-media",
        "preview",
        "open",
        "hidden",
        "smart-hidden",
    ]
    .contains(&mode)
    {
        return Err(AppError::new("invalid_input", "Unbekannter Inselzustand."));
    }
    let state = app.state::<AppState>();
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::new("conflict", "Einstellungen sind gesperrt."))?
        .clone();
    let display = state.display.snapshot(&settings)?;
    let mode = if display.fullscreen && mode != "open" {
        "hidden"
    } else {
        mode
    };
    let mut shell_target = state
        .shell_target
        .lock()
        .map_err(|_| AppError::new("conflict", "Der Inselzustand ist gesperrt."))?;
    let generation = shell_target.as_ref().map_or(0, |target| {
        target
            .generation
            .wrapping_add(u64::from(target.mode != mode))
    });
    let previous = shell_target.replace(ShellTarget {
        mode: mode.to_owned(),
        module: module.map(str::to_owned),
        generation,
    });
    drop(shell_target);
    let previous_mode = previous.as_ref().map(|target| target.mode.as_str());
    let can_animate = !settings.reduced_motion && crate::shell_motion::system_animations_enabled();
    let opening = activate && mode == "open" && previous_mode != Some("open");
    state
        .changing_shell
        .store(true, std::sync::atomic::Ordering::Release);
    let _changing = ChangingShell(&state.changing_shell);
    state
        .shell_open
        .store(mode == "open", std::sync::atomic::Ordering::Release);
    let native = window.hwnd().map_err(map_error)?;
    let fixed_top = if settings.fixed_appbar {
        let (monitor, _) =
            island_core::display::select(&display.monitors, settings.monitor.as_deref())?;
        match crate::appbar::configure(app, native, monitor, settings.island_gap() + 36.0) {
            Ok((top, changed)) => {
                if changed {
                    let _ = state.display.refresh();
                }
                Some(top)
            }
            Err(error) => {
                let _ = window.emit("service-error", error);
                None
            }
        }
    } else {
        if crate::appbar::release() {
            let _ = state.display.refresh();
        }
        None
    };
    if let Err(error) = crate::peek::configure(&window, settings.peek || display.smart_covered) {
        let _ = window.emit("service-error", error);
    }
    if opening {
        // Repeated geometry updates must not replace a remembered editor with
        // the island itself. The token validates process identity and HWND marker.
        let foreground = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        if foreground != native {
            let target = island_win::window::FocusTarget::remember(native)
                .ok()
                .flatten();
            *state
                .focus_return
                .lock()
                .map_err(|_| AppError::new("conflict", "Die Fokusrückgabe ist gesperrt."))? =
                target;
        }
    } else if mode != "open" {
        if let Some(target) = state
            .focus_return
            .lock()
            .map_err(|_| AppError::new("conflict", "Die Fokusrückgabe ist gesperrt."))?
            .take()
        {
            target.return_focus(native)?;
        }
        if matches!(mode, "hidden" | "smart-hidden") {
            crate::shell_motion::cancel();
            if mode == "smart-hidden" && can_animate && window.is_visible().map_err(map_error)? {
                if previous_mode != Some("smart-hidden") {
                    crate::clock_overlay::mark_visibility(&window, "exit");
                    let app = app.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        std::thread::sleep(std::time::Duration::from_millis(145));
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            let state = handle.state::<AppState>();
                            let still_hidden = state.shell_target.lock().is_ok_and(|target| {
                                target.as_ref().is_some_and(|target| {
                                    target.mode == "smart-hidden" && target.generation == generation
                                })
                            });
                            if still_hidden {
                                if let Some(window) = handle.get_webview_window("island") {
                                    let _ = hide_island(&window);
                                }
                            }
                        });
                    });
                }
                return Ok(());
            }
            return hide_island(&window);
        }
    }
    if previous_mode != Some(mode) {
        crate::clock_overlay::mark_visibility(
            &window,
            if previous_mode == Some("smart-hidden") && can_animate {
                "enter"
            } else {
                ""
            },
        );
    }
    let (monitor, _) =
        island_core::display::select(&display.monitors, settings.monitor.as_deref())?;
    let fixed_monitor = fixed_top.map(|top| {
        let mut monitor = monitor.clone();
        monitor.work_area.top = top.clamp(monitor.bounds.top, monitor.work_area.bottom - 1);
        monitor
    });
    let monitor = fixed_monitor.as_ref().unwrap_or(monitor);
    let scale = monitor.scale;
    let same_monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .is_some_and(|current| {
            current
                .name()
                .is_some_and(|name| name.eq_ignore_ascii_case(&monitor.device))
        })
        && (window.scale_factor().unwrap_or(0.0) - scale).abs() < 0.001;
    let notch = settings.appearance == Appearance::Notch;
    let compact = mode.starts_with("compact");
    let (width, height) = match (mode, module) {
        ("compact" | "compact-media", _) => (settings.compact_width(), 36.0),
        ("preview", _) => (320.0, 126.0),
        ("open", Some("timer")) => (600.0, 308.0),
        ("open", Some("media-player")) => (448.0, 252.0),
        _ => return Err(AppError::new("invalid_input", "Unbekannte Inselansicht.")),
    };
    let split = mode
        .starts_with("compact-")
        .then(|| island_core::display::media_bubble_layout(monitor, width, settings.island_gap()))
        .transpose()?;
    let bounds = match split.as_ref() {
        Some(layout) => layout.window,
        None => island_core::display::island_rect(monitor, width, height, settings.island_gap())?,
    };
    window.set_focusable(mode == "open").map_err(map_error)?;
    let radius = ((if compact { 18.0 } else { 24.0 }) * scale).round() as i32;
    let duration = if !animate
        || !same_monitor
        || !can_animate
        || previous_mode.is_some_and(|mode| mode.starts_with("compact-"))
        || !window.is_visible().map_err(map_error)?
    {
        0
    } else {
        match mode {
            "open" => 250,
            "preview" => 180,
            _ => 170,
        }
    };
    let frame = crate::shell_motion::Frame {
        x: bounds.left,
        y: bounds.top,
        width: bounds.right - bounds.left,
        height: bounds.bottom - bounds.top,
    };
    if let Some(layout) = split {
        // Snap between connected and split regions so a transition cannot capture the gap.
        crate::shell_motion::show_activity_bubbles(native, frame, radius, notch, layout.regions)?;
    } else {
        let curve = if mode == "open" {
            crate::shell_motion::Curve::Spring
        } else {
            crate::shell_motion::Curve::Settle
        };
        crate::shell_motion::start(window.clone(), frame, radius, notch, duration, curve)?;
    }
    window.show().map_err(map_error)?;
    if opening {
        window.set_focus().map_err(map_error)?;
    }
    island_win::shell::apply_overlay_style(window.hwnd().map_err(map_error)?)?;
    let clock_bounds = {
        let capsule = island_core::display::island_rect(
            monitor,
            settings.compact_width(),
            36.0,
            settings.island_gap(),
        )?;
        let width = (56.0 * scale).round() as i32;
        let height = (34.0 * scale).round() as i32;
        let left = capsule.left + (capsule.right - capsule.left) / 2 - width / 2;
        let top = capsule.top + if notch { 0 } else { scale.round() as i32 };
        island_core::display::Rect {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    };
    let _ = crate::clock_overlay::update(&window, Some(clock_bounds));
    let previous_module = previous
        .as_ref()
        .and_then(|target| target.module.as_deref());
    if mode == "open"
        && (opening || previous_module != module)
        && duration > 0
        && settings.clock_follows_island
    {
        crate::clock_overlay::spring(&window);
    }
    Ok(())
}

fn hide_island(window: &tauri::WebviewWindow) -> AppResult<()> {
    let _ = crate::clock_overlay::update(window, None);
    window.hide().map_err(map_error)
}

#[tauri::command]
pub async fn open_settings(app: AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || open_settings_impl(app))
        .await
        .map_err(map_error)?
}
fn open_settings_impl(app: AppHandle) -> AppResult<()> {
    // WebView2 creation must not block a synchronous command/UI event handler.
    // Serialize concurrent requests so they reuse the same settings window.
    static CREATION: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _creation = CREATION
        .lock()
        .map_err(|_| AppError::new("conflict", "Das Einstellungsfenster ist gesperrt."))?;
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(map_error)?;
        return window.set_focus().map_err(map_error);
    }
    let mut builder = WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html#settings".into()),
    )
    .title("Dynamic Island Lite – Einstellungen")
    .theme(Some(tauri::Theme::Dark))
    .visible(false)
    .inner_size(640.0, 640.0)
    .min_inner_size(480.0, 440.0);
    if let Some(root) = crate::diagnostic_root() {
        builder = builder.data_directory(root.join("webview"));
    }
    let window = builder.build().map_err(map_error)?;
    if let Ok(hwnd) = window.hwnd() {
        use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CAPTION_COLOR};
        // COLORREF is BGR. Match the settings surface (#151517).
        let color = 0x0017_1515_u32;
        let _ = unsafe {
            DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, (&color as *const u32).cast(), 4)
        };
    }
    window.show().map_err(map_error)?;
    window.set_focus().map_err(map_error)?;
    Ok(())
}
