use crate::{audio_service::AudioService, timer_service::TimerService};
use island_core::{
    error::{AppError, AppResult},
    settings::Settings,
    storage::{atomic_save, load_versioned},
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, Mutex, OnceLock,
    },
};

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub read_only: bool,
    pub warning: Option<String>,
    pub display: crate::display_service::DisplayService,
    pub timer: TimerService,
    pub audio: AudioService,
    pub media: island_win::media::MediaService,
    pub revision: Arc<AtomicU64>,
    pub hotkey: OnceLock<island_win::hotkey::Hotkey>,
    pub autostart: OnceLock<island_win::autostart::Autostart>,
    pub focus_return: Mutex<Option<island_win::window::FocusTarget>>,
    pub settings_changes: Mutex<()>,
    pub shutdown_in_progress: AtomicBool,
    pub exit_ready: AtomicBool,
    pub shell_open: AtomicBool,
    pub shell_target: Mutex<Option<crate::shell::ShellTarget>>,
    pub visualizer: crate::visualizer::Visualizer,
    pub changing_shell: AtomicBool,
}

impl AppState {
    pub fn load(settings_path: PathBuf, data_directory: PathBuf) -> AppResult<Self> {
        let (timer, timer_warning) = TimerService::load(data_directory.join("timer.json"))?;
        let warnings: Vec<_> = [timer_warning].into_iter().flatten().collect();
        match load_versioned::<Settings>(&settings_path, 1) {
            Ok(loaded) => {
                loaded.value.validate()?;
                let mut warnings = warnings;
                warnings.extend(loaded.warning);
                Ok(Self {
                    settings: Mutex::new(loaded.value),
                    settings_path,
                    read_only: false,
                    warning: if warnings.is_empty() {
                        None
                    } else {
                        Some(warnings.join("\n"))
                    },
                    display: crate::display_service::DisplayService::default(),
                    timer,
                    audio: AudioService::new()?,
                    media: island_win::media::MediaService::new(crate::media_bridge::scope())?,
                    revision: Arc::new(AtomicU64::new(0)),
                    hotkey: OnceLock::new(),
                    autostart: OnceLock::new(),
                    focus_return: Mutex::new(None),
                    settings_changes: Mutex::new(()),
                    shutdown_in_progress: AtomicBool::new(false),
                    exit_ready: AtomicBool::new(false),
                    shell_open: AtomicBool::new(false),
                    shell_target: Mutex::new(None),
                    visualizer: Default::default(),
                    changing_shell: AtomicBool::new(false),
                })
            }
            Err(error) if error.code == "unsupported" => Ok(Self {
                settings: Mutex::new(Settings::default()),
                settings_path,
                read_only: true,
                warning: Some(
                    warnings
                        .into_iter()
                        .chain([error.message])
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                display: crate::display_service::DisplayService::default(),
                timer,
                audio: AudioService::new()?,
                media: island_win::media::MediaService::new(crate::media_bridge::scope())?,
                revision: Arc::new(AtomicU64::new(0)),
                hotkey: OnceLock::new(),
                autostart: OnceLock::new(),
                focus_return: Mutex::new(None),
                settings_changes: Mutex::new(()),
                shutdown_in_progress: AtomicBool::new(false),
                exit_ready: AtomicBool::new(false),
                shell_open: AtomicBool::new(false),
                shell_target: Mutex::new(None),
                visualizer: Default::default(),
                changing_shell: AtomicBool::new(false),
            }),
            Err(error) => Err(error),
        }
    }

    pub fn initialise_autostart(&mut self, adapter: &island_win::autostart::Autostart) {
        let result = (|| -> AppResult<()> {
            let settings = self
                .settings
                .lock()
                .map_err(|_| AppError::new("conflict", "Einstellungen sind gesperrt."))?
                .clone();
            let created = settings.autostart && adapter.restore_install_registration()?;
            if let Err(error) = self.save(settings) {
                if created {
                    adapter.remove_owned_registration()?;
                }
                return Err(error);
            }
            Ok(())
        })();
        if let Err(error) = result {
            let warning = format!(
                "Der Standard für den Autostart konnte nicht gespeichert werden. {}",
                error.message
            );
            self.warning = Some(
                self.warning
                    .take()
                    .map_or(warning.clone(), |current| format!("{current}\n{warning}")),
            );
        }
    }

    pub fn save(&self, settings: Settings) -> AppResult<Settings> {
        settings.validate()?;
        if self.read_only {
            return Err(AppError::new(
                "unsupported",
                "Die neuere gespeicherte Konfiguration bleibt unverändert.",
            ));
        }
        let mut current = self.settings.lock().map_err(|_| {
            AppError::new("conflict", "Die Einstellungen sind vorübergehend gesperrt.")
        })?;
        // Publish only after durable storage succeeded.
        atomic_save(&self.settings_path, &settings)?;
        *current = settings.clone();
        Ok(settings)
    }
}
