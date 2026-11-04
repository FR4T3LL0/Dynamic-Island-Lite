use island_core::{
    error::{AppError, AppResult},
    storage::{atomic_save, load_versioned},
    timer::{Clock, TimerEngine, TimerMode, TimerState, TimerStatus},
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type Callback = Arc<dyn Fn(TimerState, u64, bool, Option<AppError>) + Send + Sync>;
struct Runtime {
    engine: TimerEngine,
    last_error: Option<AppError>,
    control_revision: u64,
}
struct Shared {
    runtime: Mutex<Runtime>,
    changed: Condvar,
    stop: AtomicBool,
    path: PathBuf,
    origin: Instant,
    read_only: bool,
    callback: Mutex<Option<Callback>>,
    publishing: Mutex<()>,
}
pub struct TimerService {
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

fn clock(origin: Instant) -> Clock {
    Clock {
        utc_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
        monotonic_ms: origin.elapsed().as_millis() as u64,
    }
}
fn poisoned() -> AppError {
    AppError::new("conflict", "Der Timer ist vorübergehend nicht verfügbar.")
}

impl TimerService {
    pub fn load(path: PathBuf) -> AppResult<(Self, Option<String>)> {
        let origin = Instant::now();
        let (state, warning, read_only) = match load_versioned::<TimerState>(&path, 1) {
            Ok(loaded) => (loaded.value, loaded.warning, false),
            Err(error) if error.code == "unsupported" => {
                (TimerState::default(), Some(error.message), true)
            }
            Err(error) => return Err(error),
        };
        let engine = TimerEngine::restore(state, clock(origin))?;
        Ok((
            Self {
                shared: Arc::new(Shared {
                    runtime: Mutex::new(Runtime {
                        engine,
                        last_error: None,
                        control_revision: 0,
                    }),
                    changed: Condvar::new(),
                    stop: AtomicBool::new(false),
                    path,
                    origin,
                    read_only,
                    callback: Mutex::new(None),
                    publishing: Mutex::new(()),
                }),
                worker: Mutex::new(None),
            },
            warning,
        ))
    }

    pub fn read_only(&self) -> bool {
        self.shared.read_only
    }
    pub fn view(&self) -> AppResult<(TimerState, u64)> {
        let runtime = self.shared.runtime.lock().map_err(|_| poisoned())?;
        Ok((runtime.engine.state().clone(), runtime.control_revision))
    }
    pub fn attach(&self, callback: Callback) -> AppResult<()> {
        let mut registered = self.shared.callback.lock().map_err(|_| poisoned())?;
        if registered.is_some() {
            return Err(AppError::new(
                "conflict",
                "Der Timerdienst wurde bereits gestartet.",
            ));
        }
        *registered = Some(callback);
        drop(registered);
        let shared = Arc::clone(&self.shared);
        *self.worker.lock().map_err(|_| poisoned())? = Some(
            std::thread::Builder::new()
                .name("island-timer".into())
                .spawn(move || Self::run(shared))?,
        );
        Ok(())
    }

    fn notify(
        shared: &Shared,
        state: TimerState,
        revision: u64,
        expired: bool,
        error: Option<AppError>,
    ) {
        let Ok(_publishing) = shared.publishing.lock() else {
            return;
        };
        let Ok(current) = shared.runtime.lock() else {
            return;
        };
        if current.control_revision != revision || current.engine.state() != &state {
            return;
        }
        drop(current);
        let callback = shared
            .callback
            .lock()
            .ok()
            .and_then(|callback| callback.clone());
        if let Some(callback) = callback {
            callback(state, revision, expired, error);
        }
    }

    fn run(shared: Arc<Shared>) {
        loop {
            let Ok(mut runtime) = shared.runtime.lock() else {
                return;
            };
            while !shared.stop.load(Ordering::Acquire)
                && (runtime.engine.state().status != TimerStatus::Running || shared.read_only)
            {
                match shared.changed.wait(runtime) {
                    Ok(next) => runtime = next,
                    Err(_) => return,
                }
            }
            if shared.stop.load(Ordering::Acquire) {
                return;
            }
            let Ok((mut runtime, _)) = shared
                .changed
                .wait_timeout(runtime, Duration::from_millis(250))
            else {
                return;
            };
            if shared.stop.load(Ordering::Acquire) {
                return;
            }
            let mut next = runtime.engine.clone();
            let outcome = next.tick(clock(shared.origin));
            if outcome.persist {
                if let Err(error) = atomic_save(&shared.path, next.state()) {
                    let first_error = runtime.last_error.as_ref() != Some(&error);
                    runtime.last_error = Some(error.clone());
                    let current = runtime.engine.state().clone();
                    let revision = runtime.control_revision;
                    drop(runtime);
                    if first_error {
                        Self::notify(&shared, current, revision, false, Some(error));
                    }
                    continue;
                }
            }
            let display_changed = next.state().status != runtime.engine.state().status
                || next.state().remaining_ms.div_ceil(1000)
                    != runtime.engine.state().remaining_ms.div_ceil(1000);
            runtime.engine = next;
            runtime.last_error = None;
            let state = runtime.engine.state().clone();
            let revision = runtime.control_revision;
            drop(runtime);
            if display_changed || outcome.expired {
                Self::notify(&shared, state, revision, outcome.expired, None);
            }
        }
    }

    fn change(
        &self,
        operation: impl FnOnce(&mut TimerEngine, Clock) -> AppResult<()>,
    ) -> AppResult<TimerState> {
        if self.shared.read_only {
            return Err(AppError::new(
                "unsupported",
                "Die neueren gespeicherten Timerdaten bleiben unverändert.",
            ));
        }
        if self.shared.stop.load(Ordering::Acquire) {
            return Err(AppError::new("conflict", "Der Timerdienst wurde beendet."));
        }
        let mut runtime = self.shared.runtime.lock().map_err(|_| poisoned())?;
        let previous = runtime.engine.state().status;
        let mut next = runtime.engine.clone();
        operation(&mut next, clock(self.shared.origin))?;
        atomic_save(&self.shared.path, next.state())?;
        runtime.engine = next;
        runtime.control_revision += 1;
        let state = runtime.engine.state().clone();
        let revision = runtime.control_revision;
        drop(runtime);
        self.shared.changed.notify_all();
        let expired = previous == TimerStatus::Running && state.status == TimerStatus::Completed;
        Self::notify(&self.shared, state.clone(), revision, expired, None);
        Ok(state)
    }
    pub fn configure(&self, mode: TimerMode, seconds: u64) -> AppResult<TimerState> {
        self.change(|engine, _| engine.configure(mode, seconds))
    }
    pub fn start(&self) -> AppResult<TimerState> {
        self.change(|engine, clock| engine.start(clock))
    }
    pub fn pause(&self) -> AppResult<TimerState> {
        self.change(|engine, clock| engine.pause(clock))
    }
    pub fn reset(&self) -> AppResult<TimerState> {
        self.change(|engine, _| {
            engine.reset();
            Ok(())
        })
    }
    pub fn shutdown(&self) {
        let guard = self.shared.runtime.lock();
        self.shared.stop.store(true, Ordering::Release);
        self.shared.changed.notify_all();
        drop(guard);
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
        if let Ok(mut callback) = self.shared.callback.lock() {
            callback.take();
        }
    }
}
impl Drop for TimerService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_worker_expires_and_durably_records_completion_without_a_webview() {
        let root = std::env::temp_dir().join(format!(
            "island-timer-worker-{}-{}",
            std::process::id(),
            clock(Instant::now()).utc_ms
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("timer.json");
        let (service, _) = TimerService::load(path.clone()).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        service
            .attach(Arc::new(move |state, _, expired, error| {
                if expired || error.is_some() {
                    let _ = tx.send((state, error));
                }
            }))
            .unwrap();
        service.configure(TimerMode::Countdown, 1).unwrap();
        service.start().unwrap();
        let (state, error) = rx.recv_timeout(Duration::from_secs(4)).unwrap();
        assert!(error.is_none());
        assert_eq!(state.status, TimerStatus::Completed);
        let persisted: TimerState = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(persisted.notified);
        assert_eq!(persisted.status, TimerStatus::Completed);
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
        service.shutdown();
        let (restored, _) = TimerService::load(path).unwrap();
        assert_eq!(restored.view().unwrap().0.status, TimerStatus::Completed);
        drop(restored);
        drop(service);
        std::fs::remove_dir_all(root).unwrap();
    }
}
