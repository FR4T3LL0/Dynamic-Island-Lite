#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appbar;
mod audio_service;
mod clock_overlay;
mod commands;
mod display_service;
mod media_bridge;
mod peek;
mod shell;
mod shell_motion;
mod state;
mod timer_service;
mod visualizer;
use island_core::{
    error::{AppError, AppResult},
    settings::Settings,
};
use serde_json::{json, Value};
use state::AppState;
use std::sync::{atomic::Ordering, Arc};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

#[tauri::command]
async fn get_snapshot(app: tauri::AppHandle) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _transaction = state
            .settings_changes
            .lock()
            .map_err(|_| AppError::new("conflict", "Einstellungen sind gesperrt."))?;
        get_snapshot_impl(&state)
    })
    .await
    .map_err(|e| AppError::new("io_error", e.to_string()))?
}
fn get_snapshot_impl(state: &AppState) -> AppResult<Value> {
    let mut capabilities = vec!["settings", "shell", "audio", "media"];
    if state.autostart.get().is_some() {
        capabilities.push("autostart");
    }
    if !state.timer.read_only() {
        capabilities.push("timer");
    }
    for _ in 0..4 {
        let revision = state.revision.load(Ordering::Acquire);
        let settings = state
            .settings
            .lock()
            .map_err(|_| AppError::new("conflict", "Einstellungen sind gesperrt."))?
            .clone();
        let display = state.display.snapshot(&settings)?;
        let (timer, timer_revision) = state.timer.view()?;
        let audio = state.audio.snapshot()?;
        let media = media_bridge::view(&state.media.snapshot()?);
        let autostart = state.autostart.get().map(|adapter| adapter.snapshot());
        let shortcut = state
            .hotkey
            .get()
            .map(|hotkey| hotkey.snapshot())
            .transpose()?
            .unwrap_or_default();
        if state.revision.load(Ordering::Acquire) != revision {
            continue;
        }
        return Ok(
            json!({ "revision": revision, "settings": settings, "autostart":autostart, "shortcut":shortcut, "display":display, "media": media.media, "mediaState":media.media_state, "audio": audio.info, "audioGeneration": audio.generation, "timer": timer, "timerRevision":timer_revision,
            "capabilities": capabilities, "warning": state.warning, "audioError": audio.error, "readOnly": state.read_only }),
        );
    }
    Err(AppError::new(
        "conflict",
        "Der Zustand wird gerade aktualisiert. Bitte erneut versuchen.",
    ))
}

#[tauri::command]
async fn save_settings(app: tauri::AppHandle, settings: Settings) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || save_settings_impl(app, settings))
        .await
        .map_err(|e| AppError::new("io_error", e.to_string()))?
}
fn save_settings_impl(app: tauri::AppHandle, mut settings: Settings) -> AppResult<Value> {
    let state = app.state::<AppState>();
    let _transaction = state
        .settings_changes
        .lock()
        .map_err(|_| AppError::new("conflict", "Die Einstellungen werden gerade geändert."))?;
    settings.shortcut = island_core::shortcut::Shortcut::parse(&settings.shortcut)?.label;
    let shortcut = settings.shortcut.clone();
    let current = state
        .settings
        .lock()
        .map_err(|_| AppError::new("conflict", "Einstellungen sind gesperrt."))?;
    let shortcut_changed = current.shortcut != shortcut;
    let autostart_changed = current.autostart != settings.autostart;
    drop(current);
    let persist = || {
        if !autostart_changed {
            return state.save(settings);
        }
        state
            .autostart
            .get()
            .ok_or_else(|| AppError::new("io_error", "Der Autostartdienst ist nicht verfügbar."))
            .and_then(|adapter| adapter.change(settings.autostart, || state.save(settings)))
    };
    let saved = if shortcut_changed {
        state
            .hotkey
            .get()
            .ok_or_else(|| {
                AppError::new("io_error", "Der Tastenkürzeldienst ist nicht verfügbar.")
            })?
            .change(&shortcut, persist)?
    } else {
        persist()?
    };
    let result = publish_settings(&app, saved)?;
    display_service::publish(&app)?;
    Ok(result)
}

fn settings_response(
    app: &tauri::AppHandle,
    settings: Settings,
    revision: u64,
) -> AppResult<Value> {
    let state = app.state::<AppState>();
    let shortcut = state
        .hotkey
        .get()
        .map(|hotkey| hotkey.snapshot())
        .transpose()?
        .unwrap_or_default();
    let autostart = state.autostart.get().map(|adapter| adapter.snapshot());
    Ok(
        json!({ "settings": settings, "autostart":autostart, "shortcut":shortcut, "revision": revision }),
    )
}

fn publish_settings(app: &tauri::AppHandle, settings: Settings) -> AppResult<Value> {
    let revision = app
        .state::<AppState>()
        .revision
        .fetch_add(1, Ordering::Relaxed)
        + 1;
    let result = settings_response(app, settings, revision)?;
    app.emit("settings-changed", &result)
        .map_err(|e| AppError::new("io_error", e.to_string()))?;
    Ok(result)
}

#[tauri::command]
async fn retry_shortcut(app: tauri::AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _transaction = state
            .settings_changes
            .lock()
            .map_err(|_| AppError::new("conflict", "Einstellungen sind gesperrt."))?;
        let settings = state
            .settings
            .lock()
            .map_err(|_| AppError::new("conflict", "Einstellungen sind gesperrt."))?
            .clone();
        let hotkey = state.hotkey.get().ok_or_else(|| {
            AppError::new("io_error", "Der Tastenkürzeldienst ist nicht verfügbar.")
        })?;
        hotkey.change(&settings.shortcut, || Ok(()))?;
        let revision = state.revision.fetch_add(1, Ordering::Relaxed) + 1;
        app.emit(
            "settings-changed",
            json!({"settings":settings,"autostart":state.autostart.get().map(|adapter|adapter.snapshot()),"shortcut":hotkey.snapshot()?,"revision":revision}),
        )
        .map_err(|e| AppError::new("io_error", e.to_string()))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::new("io_error", e.to_string()))?
}

fn diagnostic_root() -> Option<std::path::PathBuf> {
    #[cfg(all(debug_assertions, feature = "native-e2e"))]
    {
        std::env::var_os("DYNAMIC_ISLAND_TEST_ROOT").map(std::path::PathBuf::from)
    }
    #[cfg(not(all(debug_assertions, feature = "native-e2e")))]
    {
        None
    }
}

fn diagnostic_window_pid() -> Option<u32> {
    #[cfg(all(debug_assertions, feature = "native-e2e"))]
    if diagnostic_root().is_some() {
        return Some(
            std::env::var("DYNAMIC_ISLAND_TEST_WINDOW_PID")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(u32::MAX),
        );
    }
    None
}
#[tauri::command]
fn desktop_ready(app: tauri::AppHandle) -> AppResult<()> {
    app.state::<island_win::instance::Instance>().ready()
}
fn main() {
    if std::env::args().skip(1).collect::<Vec<_>>() == ["--build-info"] {
        println!(
            "{}",
            json!({"role":"dynamic-island-lite","productVersion":env!("CARGO_PKG_VERSION"),"developmentMode":tauri::is_dev(),"nativeDiagnostics":cfg!(feature="native-e2e")})
        );
        return;
    }
    let mut context = tauri::generate_context!();
    // Claim the profile before creating any WebView, service owner.
    context.config_mut().app.windows[0].create = false;
    tauri::Builder::default()
        .register_uri_scheme_protocol("island-image",|context,request|{
            let image=if request.method()==tauri::http::Method::GET {
                let app=context.app_handle();
                let key=request.uri().path().strip_prefix('/').unwrap_or("");
                app.try_state::<AppState>().and_then(|state| state.media.cover(key).ok().flatten().map(|bytes| bytes.as_ref().clone()))
            }else{None};
            let (status,bytes)=image.map_or((404,Vec::new()),|bytes|(200,bytes));
            tauri::http::Response::builder().status(status).header("Content-Type","image/png").header("Cache-Control","no-store").header("X-Content-Type-Options","nosniff").body(bytes).expect("Statische Bildantwort")
        })
        .on_window_event(|window, event| {
            if window.label()=="island" && matches!(event,tauri::WindowEvent::ScaleFactorChanged{..}) {
                if let Some(state)=window.app_handle().try_state::<AppState>(){let _=state.display.refresh();}
            }
            if window.label() == "island" && matches!(event, tauri::WindowEvent::Destroyed) {
                shell_motion::cancel();
            }
            if window.label() == "island" && matches!(event, tauri::WindowEvent::Focused(false)) {
                if let Some(state)=window.app_handle().try_state::<AppState>() {
                    let still_foreground=window.hwnd().is_ok_and(|hwnd|unsafe{windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow()}==hwnd);
                    if state.shell_open.load(Ordering::Acquire) && !state.changing_shell.load(Ordering::Acquire) && !still_foreground {
                        let _ = window.emit("shell-close", ());
                    }
                }
            }
        })
        .setup(|app| {
            let (path, data) = if let Some(root) = diagnostic_root() {
                (root.join("config/settings.json"), root.join("data"))
            } else {
                (
                    app.path().app_config_dir()?.join("settings.json"),
                    app.path().app_data_dir()?,
                )
            };
            let arguments=std::env::args_os().skip(1).collect::<Vec<_>>();
            let check_autostart=arguments.first().is_some_and(|arg|arg=="--check-install-autostart");
            let restore_autostart=arguments.first().is_some_and(|arg|arg=="--restore-install-autostart");
            if check_autostart || restore_autostart {
                if arguments.len()!=2{return Err("Der Autostartabschluss benötigt genau einen absoluten Programmpfad.".into());}
                let expected=std::path::Path::new(&arguments[1]);
                let test_name=diagnostic_root().map(|root|format!("{}{}",island_win::autostart::TEST_PREFIX,root.file_name().unwrap_or_default().to_string_lossy()));
                let adapter=island_win::autostart::Autostart::new(expected,test_name.as_deref())?;
                if restore_autostart {
                    if std::env::current_exe()?.canonicalize()?!=expected.canonicalize()?{return Err("Der Autostart darf nur für diese installierte Programmdatei wiederhergestellt werden.".into());}
                    adapter.restore_install_registration()?;app.handle().exit(0);
                } else {app.handle().exit(if adapter.is_owned_registration()?{0}else{20});}
                return Ok(());
            }
            if arguments.first().is_some_and(|arg|arg=="--finish-uninstall") {
                if arguments.len()!=2{return Err("Der Deinstallationsabschluss benötigt genau einen absoluten Programmpfad.".into());}
                let test_name=diagnostic_root().map(|root|format!("{}{}",island_win::autostart::TEST_PREFIX,root.file_name().unwrap_or_default().to_string_lossy()));
                island_win::autostart::Autostart::new(std::path::Path::new(&arguments[1]),test_name.as_deref())?.remove_owned_registration()?;
                app.handle().exit(0);return Ok(());
            }
            let prepare_install=arguments.first().is_some_and(|arg|arg=="--prepare-install");
            if prepare_install {
                if arguments.len()!=2{return Err("Die Installationsvorbereitung benötigt genau einen absoluten Programmpfad.".into());}
                island_win::instance::shutdown_existing(path.parent().ok_or("Der Konfigurationsordner fehlt.")?,std::path::Path::new(&arguments[1]),std::time::Duration::from_secs(15))?;
                app.handle().exit(0);
                return Ok(());
            }
            let activate_app=app.handle().clone();
            let shutdown_app=app.handle().clone();
            match island_win::instance::Instance::start_with_shutdown(path.parent().ok_or("Der Konfigurationsordner fehlt.")?,Arc::new(move||{
                if activate_app.try_state::<AppState>().is_some_and(|state|!state.shutdown_in_progress.load(Ordering::Acquire)) {
                    let _=activate_app.emit("shell-open",());
                }
            }),Arc::new(move||{shutdown_app.exit(0);}))? {
                island_win::instance::Start::Forwarded=>{app.handle().exit(0);return Ok(());},
                island_win::instance::Start::Primary(instance)=>{app.manage(instance);},
            }
            let first_start = !path.try_exists()?;
            let mut state = AppState::load(path, data)?;
            let test_autostart_name=diagnostic_root().map(|root|format!("{}{}",island_win::autostart::TEST_PREFIX,root.file_name().unwrap_or_default().to_string_lossy()));
            let autostart = island_win::autostart::Autostart::new(&std::env::current_exe()?, test_autostart_name.as_deref())?;
            if first_start && !tauri::is_dev() && !state.read_only {
                state.initialise_autostart(&autostart);
            }
            let _ = state.autostart.set(autostart);
            app.manage(state);
            let hotkey_app = app.handle().clone();
            let shortcut = app.state::<AppState>().settings.lock().map_err(|_|"Einstellungen sind gesperrt.")?.shortcut.clone();
            let hotkey = island_win::hotkey::Hotkey::start(&shortcut,Arc::new(move||{
                if !hotkey_app.state::<AppState>().shutdown_in_progress.load(Ordering::Acquire) {
                    let _ = hotkey_app.emit("shell-toggle",());
                }
            }))?;
            let _ = app.state::<AppState>().hotkey.set(hotkey);
            let mut island_builder=tauri::WebviewWindowBuilder::from_config(app.handle(), &app.config().app.windows[0])?;
            if let Some(root) = diagnostic_root() { island_builder=island_builder.data_directory(root.join("webview")); }
            island_builder.build()?;
            app.state::<AppState>().display.start(app.handle().clone(),diagnostic_window_pid())?;
            let audio_app = app.handle().clone();
            // An unavailable audio device must not prevent the island from starting.
            let _ = app.state::<AppState>().audio.attach(Arc::new(move |audio, generation, error| {
                let state = audio_app.state::<AppState>();
                let revision = state.revision.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = audio_app.emit("audio-updated", json!({ "audio": audio, "generation": generation, "revision": revision, "error": error }));
            }), true);
            media_bridge::attach(app.handle());
            let timer_app = app.handle().clone();
            app.state::<AppState>()
                .timer
                .attach(Arc::new(move |timer, timer_revision, expired, error| {
                    if let Some(error) = error {
                        let _ = timer_app.emit("service-error", error);
                        return;
                    }
                    let state = timer_app.state::<AppState>();
                    let revision = state.revision.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = timer_app.emit(
                        "timer-updated",
                        json!({ "timer": timer, "timerRevision":timer_revision, "revision": revision }),
                    );
                    if expired {
                        let _ = timer_app.emit("timer-expired", json!({"revision":revision}));
                        let sound = state
                            .settings
                            .lock()
                            .map(|s| s.timer_sound)
                            .unwrap_or(false);
                        if sound {
                            unsafe {
                                let _ = windows::Win32::System::Diagnostics::Debug::MessageBeep(
                                    windows::Win32::UI::WindowsAndMessaging::MB_OK,
                                );
                            }
                        }
                    }
                }))?;
            let show = MenuItem::with_id(app, "show", "Insel öffnen", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Einstellungen", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &settings, &quit])?;
            let mut builder = TrayIconBuilder::new()
                .tooltip("Dynamic Island Lite")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        let _ = app.emit("shell-open", ());
                    }
                    "settings" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = shell::open_settings(app.clone()).await {
                                let _ = app.emit("service-error", error);
                            }
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
            builder.build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,

            visualizer::set_visualizer,

            save_settings,
            retry_shortcut,
            desktop_ready,
            shell::set_shell,
            shell::shell_motion_probe,

            shell::open_settings,
            commands::timer_configure,
            commands::timer_start,
            commands::timer_pause,
            commands::timer_reset,
            commands::set_volume,

            media_bridge::media_toggle,media_bridge::media_previous,media_bridge::media_next,media_bridge::media_seek,media_bridge::refresh_media,
            display_service::refresh_displays, commands::quit_app
        ])
        .build(context)
        .expect("Dynamic Island Lite konnte nicht gestartet werden.")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested {api,..}= &event {
                if let Some(state)=app.try_state::<AppState>() {
                    if !state.exit_ready.load(Ordering::Acquire) {
                        api.prevent_exit();
                        state.shutdown_in_progress.store(true, Ordering::Release);
                        state.exit_ready.store(true, Ordering::Release);
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || app.exit(0));
                        return;
                    }
                }
            }
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                appbar::release();
                shell_motion::cancel();
                if let Some(state) = app.try_state::<AppState>() {
                    if let Some(hotkey)=state.hotkey.get(){hotkey.stop();}
                    state.timer.shutdown();
                    state.display.stop();
                    state.audio.shutdown();
                    state.media.shutdown();
                    let _ = state.visualizer.set(app.clone(), false);
                }
                if let Some(instance)=app.try_state::<island_win::instance::Instance>(){instance.stop();}
                if let tauri::RunEvent::ExitRequested {api,..} = &event {
                    let windows=app.webview_windows();
                    if !windows.is_empty() {
                        // Revoke native services while their HWNDs still exist,
                        // then let the event loop destroy the WebViews before
                        // accepting process exit. Otherwise a WebView runtime
                        // can outlive the owner and keep its profile locked.
                        api.prevent_exit();
                        for window in windows.values() {
                            if let Err(error)=window.destroy(){eprintln!("Die eigene WebView konnte nicht geschlossen werden: {error}");}
                        }
                        let app=app.clone();
                        // Destroy is queued even on the UI thread. Queue the
                        // final exit from another thread after those messages.
                        tauri::async_runtime::spawn_blocking(move||app.exit(0));
                    }
                }
            }
        });
}
