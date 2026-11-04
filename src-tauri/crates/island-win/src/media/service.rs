use super::*;
use island_core::media::{MediaAction, MediaState};
use serde::Serialize;
use std::sync::{atomic::Ordering, mpsc, Mutex};
use std::thread::JoinHandle;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSnapshot {
    #[serde(flatten)]
    pub state: MediaState,
    pub error: Option<AppError>,
    #[serde(skip)]
    pub covers: CoverPool,
    #[serde(skip)]
    pub active_subscriptions: usize,
}
pub type MediaCallback = Arc<dyn Fn(MediaSnapshot) + Send + Sync>;
type Factory =
    Box<dyn FnMut(MediaChanged, Arc<AtomicBool>) -> AppResult<Box<dyn MediaBackend>> + Send>;
enum Action {
    Enable(bool),
    Refresh,
    Select(u64, String),
    Command(u64, String, MediaAction),
}
enum Message {
    Action(Action, mpsc::SyncSender<AppResult<()>>),
    Wake(u64),
}
struct Worker {
    tx: mpsc::Sender<Message>,
    thread: JoinHandle<()>,
}
pub struct MediaService {
    worker: Mutex<Option<Worker>>,
    factory: Arc<Mutex<Factory>>,
    cached: Arc<Mutex<MediaSnapshot>>,
    callback: Arc<Mutex<Option<MediaCallback>>>,
    // Independent of the command mutex: shutdown must cancel a WinRT wait
    // while an in-flight request still owns that mutex.
    cancellation: Mutex<Option<Arc<AtomicBool>>>,
    closing: Arc<AtomicBool>,
}
fn conflict() -> AppError {
    AppError::new(
        "conflict",
        "Der Mediendienst oder seine Auswahl hat sich geändert.",
    )
}
fn refresh(
    backend: &mut dyn MediaBackend,
    snapshot: &mut MediaSnapshot,
    refresh_cover: bool,
) -> AppResult<()> {
    let next = backend.snapshot(refresh_cover)?;
    snapshot.state.update(
        snapshot.state.generation,
        next.sessions,
        next.current_id.as_deref(),
    )?;
    snapshot.covers = next.covers;
    snapshot.active_subscriptions = backend.subscriptions();
    Ok(())
}
impl MediaService {
    pub fn new(scope: SessionScope) -> AppResult<Self> {
        Self::with_backend(move |changed, cancel| {
            Ok(Box::new(MediaAdapter::new(scope.clone(), changed, cancel)?)
                as Box<dyn MediaBackend>)
        })
    }
    pub(super) fn with_backend<F>(factory: F) -> AppResult<Self>
    where
        F: FnMut(MediaChanged, Arc<AtomicBool>) -> AppResult<Box<dyn MediaBackend>>
            + Send
            + 'static,
    {
        Ok(Self {
            worker: Mutex::new(None),
            factory: Arc::new(Mutex::new(Box::new(factory))),
            cached: Arc::new(Mutex::new(MediaSnapshot::default())),
            callback: Arc::new(Mutex::new(None)),
            cancellation: Mutex::new(None),
            closing: Arc::new(AtomicBool::new(false)),
        })
    }
    fn spawn_worker(&self) -> AppResult<Worker> {
        let (tx, rx) = mpsc::channel();
        let notify = tx.clone();
        let mut current = self.snapshot()?;
        let factory = self.factory.clone();
        let target = self.cached.clone();
        let publish = self.callback.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let closing = self.closing.clone();
        // Acquire every fallible service lock before starting an owned thread.
        let mut cancellation = self.cancellation.lock().map_err(|_| conflict())?;
        let thread = std::thread::Builder::new().name("island-media".into()).spawn(move || {
            // All WinRT objects and tokens are created/closed on this MTA.
            // A stopped worker's callbacks retain only its old queue.
            let mut backend: Option<Box<dyn MediaBackend>> = None;
            let mut pending: Option<Arc<AtomicBool>> = None;
            let mut notified = false;
            let mut retry_deferred = true;
            let mut next_background = Instant::now();
            loop {
                let has_background_work = notified || (retry_deferred
                    && backend.as_ref().is_some_and(|native| native.has_pending_cover()));
                let message = if has_background_work && !closing.load(Ordering::Acquire) {
                    match rx.recv_timeout(next_background.saturating_duration_since(Instant::now())) {
                        Ok(message) => Some(message),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match rx.recv() {
                        Ok(message) => Some(message),
                        Err(_) => break,
                    }
                };
                let (action, reply) = match message {
                    Some(Message::Action(action, reply)) => {
                        retry_deferred = true;
                        (action, Some(reply))
                    }
                    Some(Message::Wake(generation)) => {
                        if current.state.enabled && generation == current.state.generation {
                            notified = true;
                        }
                        continue;
                    }
                    None => {
                        // Keep the callback coalesced until its queued notification is consumed.
                        if notified {
                            if let Some(pending) = &pending {
                                pending.store(false, Ordering::Release);
                            }
                            notified = false;
                        }
                        (Action::Refresh, None)
                    }
                };
                let background = reply.is_none();
                let stopping = matches!(action, Action::Enable(false));
                let before = current.clone();
                let result = (|| -> AppResult<()> {
                    if closing.load(Ordering::Acquire) && !stopping {return Err(conflict());}
                    if let Action::Enable(enabled) = action {
                        current.state.set_enabled(enabled)?;
                        if !enabled {
                            let closed = if let Some(native) = backend.as_mut() {
                                let result = native.close();
                                current.active_subscriptions = native.subscriptions();
                                result
                            } else {Ok(())};
                            backend = None;
                            pending = None;
                            current.covers.clear();
                            closed?;
                            if current.active_subscriptions != 0 {return Err(AppError::new("media_error","Windows hat die Medienabmeldung nicht vollständig bestätigt."));}
                            return Ok(());
                        }
                    }
                    if !current.state.enabled {return Err(conflict());}
                    if backend.is_none() {
                        let generation = current.state.generation;
                        let tx = notify.clone();
                        let dirty = Arc::new(AtomicBool::new(false));
                        let notify_dirty = dirty.clone();
                        let changed: MediaChanged = Arc::new(move || {
                            if !notify_dirty.swap(true, Ordering::AcqRel) {let _ = tx.send(Message::Wake(generation));}
                        });
                        backend = Some(factory.lock().map_err(|_| conflict())?(changed, stop.clone())?);
                        pending = Some(dirty);
                    }
                    let native = backend.as_mut().expect("backend initialized above");
                    refresh(native.as_mut(), &mut current, background)?;
                    match action {
                        Action::Select(generation, id) => {
                            if generation != current.state.generation {return Err(conflict());}
                            current.state.select(&id)?;
                        }
                        Action::Command(generation, id, action) => {
                            let command = current.state.command(generation, &id, action)?;
                            native.execute(&id, command)?;
                            refresh(native.as_mut(), &mut current, false)?;
                        }
                        _ => {}
                    }
                    Ok(())
                })();
                if background {
                    next_background = Instant::now() + BACKGROUND_REFRESH_INTERVAL;
                    // Wait for new input if reconciliation failed with work still queued.
                    retry_deferred = result.is_ok();
                }
                // Even a failed snapshot may have registered actual tokens.
                if !stopping {
                    current.active_subscriptions = backend.as_ref().map_or(0, |native| native.subscriptions());
                }
                current.error = result.as_ref().err().cloned();
                let changed = before.state != current.state || before.error != current.error || before.active_subscriptions != current.active_subscriptions;
                if let Ok(mut cached) = target.lock() {*cached = current.clone();}
                if changed {
                    let callback = publish.lock().ok().and_then(|c| c.clone());
                    if let Some(callback) = callback {callback(current.clone());}
                }
                if let Some(reply) = reply {let _ = reply.send(result);}
                if stopping {break;}
            }
            if let Some(native) = backend.as_mut() {let _ = native.close();}
            drop(backend);
        }).map_err(AppError::from)?;
        *cancellation = Some(cancel.clone());
        if self.closing.load(Ordering::Acquire) {
            cancel.store(true, Ordering::Release);
        }
        Ok(Worker { tx, thread })
    }
    pub fn attach(&self, callback: MediaCallback, enabled: bool) -> AppResult<()> {
        if self.closing.load(Ordering::Acquire) {
            return Err(conflict());
        }
        let mut registered = self.callback.lock().map_err(|_| conflict())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(conflict());
        }
        if registered.is_some() {
            return Err(AppError::new(
                "conflict",
                "Medienereignisse wurden bereits registriert.",
            ));
        }
        *registered = Some(callback);
        drop(registered);
        self.set_enabled(enabled)
    }
    pub fn snapshot(&self) -> AppResult<MediaSnapshot> {
        self.cached
            .lock()
            .map(|s| s.clone())
            .map_err(|_| conflict())
    }
    pub fn worker_running(&self) -> AppResult<bool> {
        Ok(self
            .worker
            .lock()
            .map_err(|_| conflict())?
            .as_ref()
            .is_some_and(|worker| !worker.thread.is_finished()))
    }
    pub fn cover(&self, key: &str) -> AppResult<Option<Arc<Vec<u8>>>> {
        Ok(self
            .cached
            .lock()
            .map_err(|_| conflict())?
            .covers
            .get(key)
            .cloned())
    }
    fn request(worker: &Worker, action: Action) -> AppResult<AppResult<()>> {
        let (tx, rx) = mpsc::sync_channel(1);
        worker
            .tx
            .send(Message::Action(action, tx))
            .map_err(|_| conflict())?;
        rx.recv_timeout(Duration::from_secs(10)).map_err(|_| {
            AppError::new(
                "timeout",
                "Der Mediendienst hat die Aktion noch nicht bestätigt.",
            )
        })
    }
    fn reap(worker: &mut Option<Worker>) -> AppResult<()> {
        if worker
            .as_ref()
            .is_some_and(|worker| worker.thread.is_finished())
        {
            worker
                .take()
                .expect("finished worker")
                .thread
                .join()
                .map_err(|_| {
                    AppError::new("media_error", "Der Medien-Worker wurde unerwartet beendet.")
                })?;
        }
        Ok(())
    }
    fn action(&self, action: Action) -> AppResult<()> {
        if self.closing.load(Ordering::Acquire) {
            return Err(conflict());
        }
        let mut worker = self.worker.lock().map_err(|_| conflict())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(conflict());
        }
        Self::reap(&mut worker)?;
        Self::request(worker.as_ref().ok_or_else(conflict)?, action)?
    }
    fn stop_worker(&self, worker: &mut Option<Worker>) -> AppResult<()> {
        if let Some(active) = worker.as_ref() {
            // A transport timeout retains ownership; it is not a confirmed stop.
            let result = Self::request(active, Action::Enable(false))?;
            worker
                .take()
                .expect("owned worker")
                .thread
                .join()
                .map_err(|_| {
                    AppError::new("media_error", "Der Medien-Worker wurde unerwartet beendet.")
                })?;
            *self.cancellation.lock().map_err(|_| conflict())? = None;
            result
        } else {
            let snapshot = self.snapshot()?;
            if snapshot.active_subscriptions != 0 {
                return Err(snapshot.error.unwrap_or_else(conflict));
            }
            Ok(())
        }
    }
    pub fn set_enabled(&self, enabled: bool) -> AppResult<()> {
        if self.closing.load(Ordering::Acquire) {
            return Err(conflict());
        }
        let mut worker = self.worker.lock().map_err(|_| conflict())?;
        if self.closing.load(Ordering::Acquire) {
            return Err(conflict());
        }
        Self::reap(&mut worker)?;
        if enabled {
            if worker.is_none() {
                if self.snapshot()?.active_subscriptions != 0 {
                    return Err(AppError::new("media_error","Die vorherige Medienabmeldung wurde nicht bestätigt. Bitte die Insel neu starten."));
                }
                *worker = Some(self.spawn_worker()?);
            }
            Self::request(
                worker.as_ref().expect("started worker"),
                Action::Enable(true),
            )?
        } else {
            self.stop_worker(&mut worker)
        }
    }
    pub fn refresh(&self) -> AppResult<()> {
        self.action(Action::Refresh)
    }
    pub fn select(&self, generation: u64, id: String) -> AppResult<()> {
        self.action(Action::Select(generation, id))
    }
    pub fn command(&self, generation: u64, id: String, action: MediaAction) -> AppResult<()> {
        self.action(Action::Command(generation, id, action))
    }
    pub fn shutdown(&self) {
        self.closing.store(true, Ordering::Release);
        if let Ok(cancel) = self.cancellation.lock() {
            if let Some(cancel) = cancel.as_ref() {
                cancel.store(true, Ordering::Release);
            }
        }
        if let Ok(mut worker) = self.worker.lock() {
            let _ = self.stop_worker(&mut worker);
        }
        if let Ok(mut callback) = self.callback.lock() {
            *callback = None;
        }
    }
}
impl Drop for MediaService {
    fn drop(&mut self) {
        self.shutdown();
    }
}
