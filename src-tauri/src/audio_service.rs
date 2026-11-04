use island_core::{
    audio::AudioInfo,
    error::{AppError, AppResult},
};
use island_win::audio::{AudioAdapter, AudioChanged};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

pub type AudioCallback = Arc<dyn Fn(Option<AudioInfo>, u64, Option<AppError>) + Send + Sync>;
#[derive(Clone, Default)]
pub struct AudioState {
    pub info: Option<AudioInfo>,
    pub error: Option<AppError>,
    pub generation: u64,
    pub registered_subscriptions: usize,
}
enum Action {
    Enable(bool),
    Volume(String, f32),
    Refresh,
}
enum Message {
    Action(Action, mpsc::Sender<AppResult<()>>),
    Wake,
}
type Factory = Box<dyn FnMut(AudioChanged) -> AppResult<Box<dyn AudioBackend>> + Send>;
struct Worker {
    tx: mpsc::Sender<Message>,
    thread: JoinHandle<()>,
}
#[derive(Default)]
struct Control {
    worker: Option<Worker>,
    closed: bool,
}
pub struct AudioService {
    control: Mutex<Control>,
    factory: Arc<Mutex<Factory>>,
    cached: Arc<Mutex<AudioState>>,
    callback: Arc<Mutex<Option<AudioCallback>>>,
}
trait AudioBackend {
    fn snapshot(&mut self) -> AppResult<AudioInfo>;
    fn set_volume(&mut self, id: &str, volume: f32) -> AppResult<AudioInfo>;
    fn close(&mut self) -> AppResult<()> {
        Ok(())
    }
    fn subscription_count(&self) -> usize {
        0
    }
}
impl AudioBackend for AudioAdapter {
    fn snapshot(&mut self) -> AppResult<AudioInfo> {
        AudioAdapter::snapshot(self)
    }
    fn set_volume(&mut self, id: &str, volume: f32) -> AppResult<AudioInfo> {
        AudioAdapter::set_volume(self, id, volume)
    }
    fn close(&mut self) -> AppResult<()> {
        AudioAdapter::close(self)
    }
    fn subscription_count(&self) -> usize {
        AudioAdapter::subscription_count(self)
    }
}
fn conflict() -> AppError {
    AppError::new("conflict", "Der Audiodienst ist deaktiviert oder beendet.")
}
impl AudioService {
    pub fn new() -> AppResult<Self> {
        Self::with_backend(|changed| {
            AudioAdapter::new(Some(changed))
                .map(|adapter| Box::new(adapter) as Box<dyn AudioBackend>)
        })
    }
    fn with_backend<F>(factory: F) -> AppResult<Self>
    where
        F: FnMut(AudioChanged) -> AppResult<Box<dyn AudioBackend>> + Send + 'static,
    {
        Ok(Self {
            control: Mutex::new(Control::default()),
            factory: Arc::new(Mutex::new(Box::new(factory))),
            cached: Arc::new(Mutex::new(AudioState::default())),
            callback: Arc::new(Mutex::new(None)),
        })
    }
    fn spawn_worker(&self) -> AppResult<Worker> {
        let (tx, rx) = mpsc::channel();
        // Every worker owns its queue and dirty bit. An old COM callback can
        // never wake the next generation, even if delivered after unregister.
        let dirty = Arc::new(AtomicBool::new(false));
        let pending = dirty.clone();
        let notify = tx.clone();
        let changed: AudioChanged = Arc::new(move || {
            if !pending.swap(true, Ordering::AcqRel) {
                let _ = notify.send(Message::Wake);
            }
        });
        let factory = self.factory.clone();
        let cached = self.cached.clone();
        let callback = self.callback.clone();
        let mut generation = self.snapshot()?.generation;
        let thread = std::thread::Builder::new()
            .name("island-audio".into())
            .spawn(move || {
                let mut adapter: Option<Box<dyn AudioBackend>> = None;
                let mut enabled = false;
                while let Ok(message) = rx.recv() {
                    let (action, reply) = match message {
                        Message::Action(action, reply) => (action, Some(reply)),
                        Message::Wake => {
                            dirty.store(false, Ordering::Release);
                            if !enabled {
                                continue;
                            }
                            (Action::Refresh, None)
                        }
                    };
                    let stopping = matches!(action, Action::Enable(false));
                    let mut remaining = 0;
                    let result = (|| -> AppResult<Option<AudioInfo>> {
                        if let Action::Enable(next) = action {
                            if next != enabled {
                                generation += 1;
                            }
                            enabled = next;
                            if !enabled {
                                let closed = if let Some(native) = adapter.as_mut() {
                                    remaining = native.subscription_count();
                                    native.close()
                                } else {
                                    Ok(())
                                };
                                if closed.is_ok() {
                                    remaining = adapter
                                        .as_ref()
                                        .map_or(0, |native| native.subscription_count());
                                }
                                // Drop on the COM owner before acknowledging the stop.
                                adapter = None;
                                closed?;
                                return Ok(None);
                            }
                        }
                        if !enabled {
                            return Err(conflict());
                        }
                        if adapter.is_none() {
                            adapter =
                                Some(factory.lock().map_err(|_| conflict())?(changed.clone())?);
                        }
                        let native = adapter.as_mut().expect("audio adapter initialized");
                        Ok(Some(match action {
                            Action::Volume(id, value) => native.set_volume(&id, value)?,
                            Action::Enable(_) | Action::Refresh => native.snapshot()?,
                        }))
                    })();
                    let (info, error) = match &result {
                        Ok(info) => (info.clone(), None),
                        Err(error) => (
                            adapter.as_mut().and_then(|native| native.snapshot().ok()),
                            Some(error.clone()),
                        ),
                    };
                    let subscriptions = if stopping {
                        remaining
                    } else {
                        adapter
                            .as_ref()
                            .map_or(0, |native| native.subscription_count())
                    };
                    let next = AudioState {
                        info,
                        error,
                        generation,
                        registered_subscriptions: subscriptions,
                    };
                    Self::publish(&cached, &callback, next);
                    if let Some(reply) = reply {
                        let _ = reply.send(result.map(|_| ()));
                    }
                    if stopping {
                        break;
                    }
                }
                // The last COM objects and registration tokens stay on this thread.
                if let Some(native) = adapter.as_mut() {
                    let _ = native.close();
                }
                drop(adapter);
            })
            .map_err(AppError::from)?;
        Ok(Worker { tx, thread })
    }
    fn publish(
        cached: &Mutex<AudioState>,
        callback: &Mutex<Option<AudioCallback>>,
        next: AudioState,
    ) {
        let changed = if let Ok(mut current) = cached.lock() {
            let changed = current.info != next.info
                || current.error != next.error
                || current.generation != next.generation
                || current.registered_subscriptions != next.registered_subscriptions;
            *current = next.clone();
            changed
        } else {
            false
        };
        if changed {
            let callback = callback.lock().ok().and_then(|callback| callback.clone());
            if let Some(callback) = callback {
                callback(next.info, next.generation, next.error);
            }
        }
    }
    fn request(worker: &Worker, action: Action) -> AppResult<AppResult<()>> {
        let (tx, rx) = mpsc::channel();
        worker
            .tx
            .send(Message::Action(action, tx))
            .map_err(|_| conflict())?;
        rx.recv_timeout(Duration::from_secs(5)).map_err(|_| {
            AppError::new(
                "timeout",
                "Windows hat die Audioaktion noch nicht bestätigt. Bitte den Zustand erneut laden.",
            )
        })
    }
    fn reap(control: &mut Control) -> AppResult<()> {
        if control
            .worker
            .as_ref()
            .is_some_and(|worker| worker.thread.is_finished())
        {
            if let Some(worker) = control.worker.take() {
                worker.thread.join().map_err(|_| {
                    AppError::new("io_error", "Der Audio-Worker wurde unerwartet beendet.")
                })?;
            }
        }
        Ok(())
    }
    pub fn attach(&self, callback: AudioCallback, enabled: bool) -> AppResult<()> {
        let mut registered = self.callback.lock().map_err(|_| conflict())?;
        if registered.is_some() {
            return Err(AppError::new(
                "conflict",
                "Audioereignisse wurden bereits registriert.",
            ));
        }
        *registered = Some(callback);
        drop(registered);
        self.set_enabled(enabled)
    }
    pub fn snapshot(&self) -> AppResult<AudioState> {
        self.cached
            .lock()
            .map(|state| state.clone())
            .map_err(|_| conflict())
    }
    #[cfg(test)]
    pub fn worker_running(&self) -> AppResult<bool> {
        Ok(self
            .control
            .lock()
            .map_err(|_| conflict())?
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.thread.is_finished()))
    }
    fn action(&self, action: Action) -> AppResult<()> {
        let mut control = self.control.lock().map_err(|_| conflict())?;
        if control.closed {
            return Err(conflict());
        }
        Self::reap(&mut control)?;
        Self::request(control.worker.as_ref().ok_or_else(conflict)?, action)?
    }
    pub fn set_enabled(&self, enabled: bool) -> AppResult<()> {
        let mut control = self.control.lock().map_err(|_| conflict())?;
        if control.closed {
            return Err(conflict());
        }
        Self::reap(&mut control)?;
        if enabled {
            if self.snapshot()?.registered_subscriptions > 0 && control.worker.is_none() {
                return Err(AppError::new("audio_error","Die vorherige Audioabmeldung wurde nicht bestätigt. Bitte die Insel neu starten."));
            }
            if control.worker.is_none() {
                control.worker = Some(self.spawn_worker()?);
            }
            Self::request(
                control.worker.as_ref().expect("worker started"),
                Action::Enable(true),
            )?
        } else if let Some(worker) = control.worker.as_ref() {
            // A transport timeout retains ownership; never detach an unknown
            // still-running worker or claim that stop has completed.
            let result = Self::request(worker, Action::Enable(false))?;
            let worker = control.worker.take().expect("owned audio worker");
            worker.thread.join().map_err(|_| {
                AppError::new("io_error", "Der Audio-Worker wurde unerwartet beendet.")
            })?;
            result
        } else {
            Ok(())
        }
    }
    #[cfg(test)]
    pub fn refresh(&self) -> AppResult<()> {
        self.action(Action::Refresh)
    }
    pub fn set_volume(&self, id: String, volume: f32) -> AppResult<()> {
        self.action(Action::Volume(id, volume))
    }
    pub fn shutdown(&self) {
        let _ = self.set_enabled(false);
        if let Ok(mut control) = self.control.lock() {
            control.closed = true;
        }
        if let Ok(mut callback) = self.callback.lock() {
            *callback = None;
        }
    }
}
impl Drop for AudioService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use island_core::audio::AudioDevice;
    use std::sync::atomic::AtomicUsize;
    struct Backend {
        state: Arc<Mutex<AudioInfo>>,
        live: Arc<AtomicUsize>,
        reads: Arc<AtomicUsize>,
        close_error: Arc<AtomicBool>,
        closed: bool,
    }
    impl Drop for Backend {
        fn drop(&mut self) {
            self.live.fetch_sub(1, Ordering::AcqRel);
        }
    }
    impl AudioBackend for Backend {
        fn close(&mut self) -> AppResult<()> {
            if self.close_error.load(Ordering::Acquire) {
                return Err(AppError::new(
                    "audio_error",
                    "Erzwungener eigener Abmeldefehler.",
                ));
            }
            self.closed = true;
            Ok(())
        }
        fn subscription_count(&self) -> usize {
            if self.closed {
                0
            } else {
                2
            }
        }
        fn snapshot(&mut self) -> AppResult<AudioInfo> {
            self.reads.fetch_add(1, Ordering::AcqRel);
            Ok(self.state.lock().unwrap().clone())
        }
        fn set_volume(&mut self, _: &str, _: f32) -> AppResult<AudioInfo> {
            Err(AppError::new("access_denied", "Gerät wurde entfernt."))
        }
    }
    #[test]
    fn disabled_start_and_restart_own_exactly_one_worker() {
        let (service, attempts) = model_service(false, false);
        service.attach(Arc::new(|_, _, _| {}), false).unwrap();
        assert!(!service.worker_running().unwrap());
        assert_eq!(attempts.load(Ordering::Acquire), 0);
        service.set_enabled(true).unwrap();
        let first = worker_id(&service);
        service.set_enabled(true).unwrap();
        assert_eq!(worker_id(&service), first);
        assert_eq!(attempts.load(Ordering::Acquire), 1);
        assert_eq!(service.snapshot().unwrap().generation, 1);
        service.set_enabled(false).unwrap();
        service.set_enabled(false).unwrap();
        assert!(!service.worker_running().unwrap());
        assert_eq!(service.snapshot().unwrap().registered_subscriptions, 0);
        service.set_enabled(true).unwrap();
        assert_ne!(worker_id(&service), first);
        assert_eq!(service.snapshot().unwrap().generation, 3);
        assert_eq!(attempts.load(Ordering::Acquire), 2);
        service.shutdown();
        assert!(!service.worker_running().unwrap());
        assert!(service.set_enabled(true).is_err());
    }
    #[test]
    fn failed_start_retries_on_the_same_worker_and_confirmed_close_ends_it() {
        let (service, attempts) = model_service(false, true);
        assert!(service.attach(Arc::new(|_, _, _| {}), true).is_err());
        assert!(service.worker_running().unwrap());
        let first = worker_id(&service);
        assert!(service.snapshot().unwrap().error.is_some());
        service.refresh().unwrap();
        assert_eq!(worker_id(&service), first);
        assert_eq!(attempts.load(Ordering::Acquire), 2);
        assert!(service.snapshot().unwrap().error.is_none());
        service.set_enabled(false).unwrap();
        assert!(!service.worker_running().unwrap());
        assert_eq!(service.snapshot().unwrap().registered_subscriptions, 0);
    }
    #[test]
    fn failed_unregistration_is_reported_and_never_claimed_clean() {
        let (service, attempts) = model_service(true, false);
        service.attach(Arc::new(|_, _, _| {}), true).unwrap();
        assert_eq!(service.set_enabled(false).unwrap_err().code, "audio_error");
        assert!(!service.worker_running().unwrap());
        let state = service.snapshot().unwrap();
        assert!(state.info.is_none());
        assert!(state.error.is_some());
        assert_eq!(state.registered_subscriptions, 2);
        assert!(service.set_enabled(true).is_err());
        assert_eq!(attempts.load(Ordering::Acquire), 1);
    }
    fn worker_id(service: &AudioService) -> std::thread::ThreadId {
        service
            .control
            .lock()
            .unwrap()
            .worker
            .as_ref()
            .unwrap()
            .thread
            .thread()
            .id()
    }
    fn model_service(close_error: bool, first_error: bool) -> (AudioService, Arc<AtomicUsize>) {
        let attempts = Arc::new(AtomicUsize::new(0));
        let calls = attempts.clone();
        let service = AudioService::with_backend(move |_| {
            let attempt = calls.fetch_add(1, Ordering::AcqRel);
            if first_error && attempt == 0 {
                return Err(AppError::new("audio_error", "Eigener Startfehler."));
            }
            let live = Arc::new(AtomicUsize::new(1));
            Ok(Box::new(Backend {
                state: Arc::new(Mutex::new(AudioInfo {
                    volume: 42.0,
                    muted: false,
                    default_device: "one".into(),
                    devices: vec![AudioDevice {
                        id: "one".into(),
                        name: "Eigene Ausgabe".into(),
                    }],
                })),
                live,
                reads: Arc::new(AtomicUsize::new(0)),
                close_error: Arc::new(AtomicBool::new(close_error)),
                closed: false,
            }))
        })
        .unwrap();
        (service, attempts)
    }
    #[test]
    fn hotplug_failure_readback_and_ten_lifecycles_do_not_leak() {
        let info = Arc::new(Mutex::new(AudioInfo {
            volume: 42.0,
            muted: false,
            default_device: "one".into(),
            devices: vec![AudioDevice {
                id: "one".into(),
                name: "Erste Ausgabe".into(),
            }],
        }));
        let live = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let close_error = Arc::new(AtomicBool::new(false));
        let notifier = Arc::new(Mutex::new(None::<AudioChanged>));
        let state_copy = info.clone();
        let live_copy = live.clone();
        let notify_copy = notifier.clone();
        let read_copy = reads.clone();
        let service = AudioService::with_backend(move |changed| {
            *notify_copy.lock().unwrap() = Some(changed);
            live_copy.fetch_add(1, Ordering::AcqRel);
            Ok(Box::new(Backend {
                state: state_copy.clone(),
                live: live_copy.clone(),
                reads: read_copy.clone(),
                close_error: close_error.clone(),
                closed: false,
            }))
        })
        .unwrap();
        let (tx, rx) = mpsc::channel();
        service
            .attach(
                Arc::new(move |info, generation, error| {
                    let _ = tx.send((info, generation, error));
                }),
                true,
            )
            .unwrap();
        assert_eq!(live.load(Ordering::Acquire), 1);
        let first_generation = service.snapshot().unwrap().generation;
        assert!(service.set_volume("one".into(), 90.0).is_err());
        let after_failure = service.snapshot().unwrap();
        assert_eq!(after_failure.info.unwrap().volume, 42.0);
        assert_eq!(after_failure.error.unwrap().code, "access_denied");
        *info.lock().unwrap() = AudioInfo {
            volume: 0.0,
            muted: false,
            default_device: String::new(),
            devices: vec![],
        };
        let notify = notifier.lock().unwrap().as_ref().unwrap().clone();
        notify();
        loop {
            let (state, _, error) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
            if state.is_some_and(|s| s.devices.is_empty()) {
                assert!(error.is_none());
                break;
            }
        }
        assert!(service
            .snapshot()
            .unwrap()
            .info
            .unwrap()
            .default_device
            .is_empty());
        for _ in 0..10 {
            service.set_enabled(false).unwrap();
            service.set_enabled(false).unwrap();
            assert_eq!(live.load(Ordering::Acquire), 0);
            assert_eq!(service.snapshot().unwrap().registered_subscriptions, 0);
            assert!(
                !service.worker_running().unwrap(),
                "Ein freigegebener Adapter darf keinen Empfängerthread zurücklassen"
            );
            service.set_enabled(true).unwrap();
            service.set_enabled(true).unwrap();
            assert_eq!(live.load(Ordering::Acquire), 1);
            assert_eq!(service.snapshot().unwrap().registered_subscriptions, 2);
        }
        assert_eq!(
            service.snapshot().unwrap().generation,
            first_generation + 20
        );
        let before_reads = reads.load(Ordering::Acquire);
        notify();
        service.refresh().unwrap();
        assert_eq!(
            reads.load(Ordering::Acquire),
            before_reads + 1,
            "Eine alte Callbackgeneration darf den neuen Worker nicht wecken"
        );
        service.shutdown();
        assert_eq!(live.load(Ordering::Acquire), 0);
        assert!(service.refresh().is_err());
    }
}
