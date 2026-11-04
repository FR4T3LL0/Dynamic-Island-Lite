use super::*;
use island_core::media::{MediaAction, MediaCapabilities, MediaTimeline};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
#[derive(Default)]
struct Shared {
    state: Mutex<BackendSnapshot>,
    commands: Mutex<Vec<(String, MediaCommand)>>,
    callbacks: Mutex<Vec<MediaChanged>>,
    live: AtomicUsize,
    reads: AtomicUsize,
    reject: AtomicBool,
    fail_background: AtomicBool,
    close_error: AtomicBool,
    closes: AtomicUsize,
    worker_ids: Mutex<Vec<std::thread::ThreadId>>,
    pending_cover: Mutex<Option<u8>>,
    cover_reads: Mutex<Vec<(Instant, u8)>>,
}
struct Backend(Arc<Shared>, bool);
impl Drop for Backend {
    fn drop(&mut self) {
        self.0.live.fetch_sub(1, Ordering::AcqRel);
    }
}
impl MediaBackend for Backend {
    fn close(&mut self) -> AppResult<()> {
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        if self.0.close_error.load(Ordering::Acquire) {
            return Err(AppError::new(
                "media_error",
                "Eigener simulierter Abmeldefehler.",
            ));
        }
        self.1 = true;
        Ok(())
    }
    fn subscriptions(&self) -> usize {
        if self.1 {
            0
        } else {
            8
        }
    }
    fn has_pending_cover(&self) -> bool {
        self.0.pending_cover.lock().unwrap().is_some()
    }
    fn snapshot(&mut self, refresh_cover: bool) -> AppResult<BackendSnapshot> {
        self.0.reads.fetch_add(1, Ordering::AcqRel);
        if refresh_cover && self.0.fail_background.load(Ordering::Acquire) {
            return Err(AppError::new("media_error", "Eigener simulierter Aktualisierungsfehler."));
        }
        let mut state = self.0.state.lock().unwrap();
        if refresh_cover {
            if let Some(value) = self.0.pending_cover.lock().unwrap().take() {
                let key = format!("cover-{value}");
                state.sessions[0].cover_key = Some(key.clone());
                state.covers.clear();
                state.covers.insert(key, Arc::new(vec![value]));
                self.0
                    .cover_reads
                    .lock()
                    .unwrap()
                    .push((Instant::now(), value));
            }
        }
        Ok(state.clone())
    }
    fn execute(&mut self, id: &str, command: MediaCommand) -> AppResult<()> {
        if self.0.reject.load(Ordering::Acquire) {
            return Err(AppError::new(
                "media_error",
                "Eigener simulierter Playerfehler.",
            ));
        }
        self.0.commands.lock().unwrap().push((id.into(), command));
        Ok(())
    }
}
fn session(id: &str) -> MediaSession {
    MediaSession {
        id: id.into(),
        app: "Eigener Player".into(),
        app_label: None,
        title: "Titel".into(),
        artist: String::new(),
        playing: false,
        cover_key: None,
        cover_tint: None,
        timeline: MediaTimeline {
            position_ms: 1000,
            duration_ms: 60000,
            seek_start_ms: 0,
            seek_end_ms: 60000,
            sampled_at_ms: 0,
            rate: 1.,
        },
        capabilities: MediaCapabilities {
            toggle: true,
            seek: true,
            ..Default::default()
        },
    }
}
fn setup() -> (MediaService, Arc<Shared>) {
    let shared = Arc::new(Shared::default());
    shared.state.lock().unwrap().sessions = vec![session("one"), session("two")];
    let native = shared.clone();
    let service = MediaService::with_backend(move |callback, _| {
        native.callbacks.lock().unwrap().push(callback);
        native
            .worker_ids
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        native.live.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(Backend(native.clone(), false)) as Box<dyn MediaBackend>)
    })
    .unwrap();
    (service, shared)
}

#[test]
fn cover_bursts_are_limited_and_the_last_cover_is_published_without_another_event() {
    let (service, shared) = setup();
    service.set_enabled(true).unwrap();
    let generation = service.snapshot().unwrap().state.generation;
    let notify = shared.callbacks.lock().unwrap()[0].clone();
    let wait_for_cover = |value| {
        let key = format!("cover-{value}");
        let deadline = Instant::now() + Duration::from_secs(3);
        while service.cover(&key).unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "Cover {value} wurde nicht veröffentlicht"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    };

    *shared.pending_cover.lock().unwrap() = Some(1);
    notify();
    wait_for_cover(1);
    for value in 2..=100 {
        *shared.pending_cover.lock().unwrap() = Some(value);
        notify();
    }
    service
        .command(generation, "one".into(), MediaAction::Toggle)
        .unwrap();
    assert_eq!(shared.commands.lock().unwrap().len(), 1);
    wait_for_cover(100);

    // A foreground refresh can discover cover work without a WinRT wake.
    *shared.pending_cover.lock().unwrap() = Some(101);
    service.refresh().unwrap();
    wait_for_cover(101);
    let reads = shared.cover_reads.lock().unwrap();
    assert!(reads.windows(2).all(|pair| {
        pair[1].0.duration_since(pair[0].0) >= BACKGROUND_REFRESH_INTERVAL
    }));
    assert_eq!(reads.last().unwrap().1, 101);
}

#[test]
fn failed_background_refresh_waits_for_new_input_before_retrying() {
    let (service, shared) = setup();
    service.set_enabled(true).unwrap();
    let notify = shared.callbacks.lock().unwrap()[0].clone();
    shared.fail_background.store(true, Ordering::Release);
    *shared.pending_cover.lock().unwrap() = Some(1);
    service.refresh().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while service.snapshot().unwrap().error.is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let reads = shared.reads.load(Ordering::Acquire);
    std::thread::sleep(BACKGROUND_REFRESH_INTERVAL * 2);
    assert_eq!(shared.reads.load(Ordering::Acquire), reads);
    shared.fail_background.store(false, Ordering::Release);
    notify();
    let deadline = Instant::now() + Duration::from_secs(3);
    while service.cover("cover-1").unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn service_lifecycle_clears_covers_and_discards_late_generations_without_polling() {
    let (service, shared) = setup();
    {
        let mut state = shared.state.lock().unwrap();
        state.sessions[0].cover_key = Some("own-cover".into());
        state
            .covers
            .insert("own-cover".into(), Arc::new(vec![1, 2, 3]));
    }
    assert!(!service.worker_running().unwrap());
    assert!(shared.worker_ids.lock().unwrap().is_empty());
    let mut previous = 0;
    for _ in 0..10 {
        service.set_enabled(true).unwrap();
        let current = service.snapshot().unwrap().state;
        assert!(current.generation > previous);
        previous = current.generation;
        service.set_enabled(true).unwrap();
        assert_eq!(service.snapshot().unwrap().state.generation, previous);
        assert_eq!(shared.live.load(Ordering::Acquire), 1);
        assert!(service.worker_running().unwrap());
        assert_eq!(
            service.cover("own-cover").unwrap().unwrap().as_slice(),
            &[1, 2, 3]
        );
        assert_eq!(service.snapshot().unwrap().active_subscriptions, 8);
        let reads = shared.reads.load(Ordering::Acquire);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(shared.reads.load(Ordering::Acquire), reads);
        service.set_enabled(false).unwrap();
        service.set_enabled(false).unwrap();
        let stopped = service.snapshot().unwrap();
        assert!(!stopped.state.enabled);
        assert!(stopped.state.sessions.is_empty());
        assert!(stopped.covers.is_empty());
        assert!(service.cover("own-cover").unwrap().is_none());
        assert_eq!(stopped.active_subscriptions, 0);
        assert_eq!(shared.live.load(Ordering::Acquire), 0);
        assert!(
            !service.worker_running().unwrap(),
            "Ein deaktivierter Mediendienst darf keinen Empfängerthread behalten"
        );
        let stale = shared.callbacks.lock().unwrap().last().unwrap().clone();
        service.set_enabled(true).unwrap();
        let reads = shared.reads.load(Ordering::Acquire);
        stale();
        service.refresh().unwrap();
        assert_eq!(shared.reads.load(Ordering::Acquire), reads + 1);
        service.set_enabled(false).unwrap();
    }
    service.shutdown();
    service.shutdown();
    assert_eq!(shared.live.load(Ordering::Acquire), 0);
    assert_eq!(shared.closes.load(Ordering::Acquire), 20);
    let worker_ids = shared.worker_ids.lock().unwrap();
    assert_eq!(worker_ids.len(), 20);
    let unique: std::collections::HashSet<_> = worker_ids.iter().collect();
    assert_eq!(
        unique.len(),
        20,
        "Jeder Neustart muss einen neuen tatsächlichen Worker besitzen"
    );
    assert!(service.refresh().is_err());
}
#[test]
fn remote_commands_revalidate_selection_capabilities_and_generation_without_optimism() {
    let (service, shared) = setup();
    service.set_enabled(true).unwrap();
    let generation = service.snapshot().unwrap().state.generation;
    service
        .command(generation, "one".into(), MediaAction::Toggle)
        .unwrap();
    assert!(!service.snapshot().unwrap().state.sessions[0].playing);
    assert_eq!(
        shared.commands.lock().unwrap().as_slice(),
        &[("one".into(), MediaCommand::Toggle)]
    );
    service.select(generation, "two".into()).unwrap();
    assert!(service
        .command(generation, "one".into(), MediaAction::Toggle)
        .is_err());
    assert!(service
        .command(generation, "two".into(), MediaAction::Next)
        .is_err());
    assert!(service
        .command(generation, "two".into(), MediaAction::Seek(60001))
        .is_err());
    shared
        .state
        .lock()
        .unwrap()
        .sessions
        .retain(|s| s.id != "two");
    assert!(service
        .command(generation, "two".into(), MediaAction::Toggle)
        .is_err());
    service.set_enabled(false).unwrap();
    service.set_enabled(true).unwrap();
    assert!(service
        .command(generation, "one".into(), MediaAction::Toggle)
        .is_err());
    assert_eq!(shared.commands.lock().unwrap().len(), 1);
}
#[test]
fn native_notifications_confirm_state_and_player_failure_is_not_a_success() {
    let (service, shared) = setup();
    let (tx, rx) = std::sync::mpsc::channel();
    service
        .attach(
            Arc::new(move |state| {
                let _ = tx.send(state);
            }),
            true,
        )
        .unwrap();
    rx.recv_timeout(Duration::from_secs(1)).unwrap();
    shared.state.lock().unwrap().sessions[0].playing = true;
    shared.callbacks.lock().unwrap()[0]();
    let updated = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(updated.state.sessions[0].playing);
    assert!(updated.error.is_none());
    shared.reject.store(true, Ordering::Release);
    assert!(service
        .command(updated.state.generation, "one".into(), MediaAction::Toggle)
        .is_err());
    let failed = service.snapshot().unwrap();
    assert!(failed.state.sessions[0].playing);
    assert_eq!(failed.error.unwrap().code, "media_error");
    shared.state.lock().unwrap().sessions.clear();
    shared.callbacks.lock().unwrap()[0]();
    let deadline = Instant::now() + Duration::from_secs(1);
    while !service.snapshot().unwrap().state.sessions.is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(service.snapshot().unwrap().state.selected_id.is_none());
}

#[test]
fn asynchronous_timeout_and_shutdown_cancel_inflight_work() {
    let calls = AtomicUsize::new(0);
    let cancel = AtomicBool::new(false);
    let result = wait_status(
        || Ok(AsyncStatus::Started),
        || {
            calls.fetch_add(1, Ordering::AcqRel);
            Ok(())
        },
        &cancel,
        Instant::now(),
    );
    assert_eq!(result.unwrap_err().code, "timeout");
    assert_eq!(calls.load(Ordering::Acquire), 1);
    cancel.store(true, Ordering::Release);
    let result = wait_status(
        || panic!("A cancelled wait must not query COM"),
        || {
            calls.fetch_add(1, Ordering::AcqRel);
            Ok(())
        },
        &cancel,
        Instant::now() + Duration::from_secs(5),
    );
    assert_eq!(result.unwrap_err().code, "timeout");
    assert_eq!(calls.load(Ordering::Acquire), 2);
}

#[test]
fn disabled_start_creation_failure_retry_and_idempotent_enable_keep_one_worker() {
    let shared = Arc::new(Shared::default());
    let native = shared.clone();
    let attempts = Arc::new(AtomicUsize::new(0));
    let calls = attempts.clone();
    let threads = Arc::new(Mutex::new(Vec::new()));
    let ids = threads.clone();
    let service = MediaService::with_backend(move |callback, _| {
        ids.lock().unwrap().push(std::thread::current().id());
        if calls.fetch_add(1, Ordering::AcqRel) == 0 {
            return Err(AppError::new("media_error", "Eigener Startfehler."));
        }
        native.callbacks.lock().unwrap().push(callback);
        native.live.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(Backend(native.clone(), false)) as Box<dyn MediaBackend>)
    })
    .unwrap();
    service.attach(Arc::new(|_| {}), false).unwrap();
    assert_eq!(attempts.load(Ordering::Acquire), 0);
    assert!(!service.worker_running().unwrap());
    assert!(service.attach(Arc::new(|_| {}), false).is_err());
    assert!(service.set_enabled(true).is_err());
    assert!(service.worker_running().unwrap());
    let failed = service.snapshot().unwrap();
    assert!(failed.state.enabled);
    assert_eq!(failed.error.unwrap().code, "media_error");
    service.refresh().unwrap();
    assert!(service.snapshot().unwrap().error.is_none());
    service.set_enabled(true).unwrap();
    assert_eq!(attempts.load(Ordering::Acquire), 2);
    assert_eq!(
        service.snapshot().unwrap().state.generation,
        failed.state.generation
    );
    {
        let observed = threads.lock().unwrap();
        assert_eq!(observed[0], observed[1]);
    }
    service.set_enabled(false).unwrap();
    assert!(!service.worker_running().unwrap());
    service.set_enabled(true).unwrap();
    assert!(service.worker_running().unwrap());
    {
        let observed = threads.lock().unwrap();
        assert_ne!(observed[1], observed[2]);
    }
    service.shutdown();
    service.shutdown();
    assert!(!service.worker_running().unwrap());
    assert!(service.set_enabled(true).is_err());
    assert_eq!(shared.live.load(Ordering::Acquire), 0);
}

#[test]
fn failed_unregistration_never_claims_clean_or_starts_another_worker() {
    let (service, shared) = setup();
    service.set_enabled(true).unwrap();
    shared.close_error.store(true, Ordering::Release);
    let failure = service.set_enabled(false).unwrap_err();
    assert_eq!(failure.code, "media_error");
    let stopped = service.snapshot().unwrap();
    assert!(!stopped.state.enabled);
    assert!(stopped.state.sessions.is_empty());
    assert_eq!(stopped.active_subscriptions, 8);
    assert_eq!(stopped.error.unwrap().code, "media_error");
    assert!(!service.worker_running().unwrap());
    assert_eq!(shared.live.load(Ordering::Acquire), 0);
    assert!(service.set_enabled(false).is_err());
    assert!(service.set_enabled(true).is_err());
    assert_eq!(shared.worker_ids.lock().unwrap().len(), 1);
    service.shutdown();
    assert_eq!(service.snapshot().unwrap().active_subscriptions, 8);
}

#[test]
fn shutdown_cancels_creation_without_waiting_for_the_command_mutex() {
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
    let service = Arc::new(
        MediaService::with_backend(move |_, cancel| {
            started_tx.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while !cancel.load(Ordering::Acquire) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(
                cancel.load(Ordering::Acquire),
                "Shutdown muss den laufenden WinRT-Wartepfad vor dem Befehls-Lock abbrechen"
            );
            Err(AppError::new(
                "timeout",
                "Eigene abgebrochene Initialisierung.",
            ))
        })
        .unwrap(),
    );
    let starting = service.clone();
    let request = std::thread::spawn(move || starting.set_enabled(true));
    started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    service.shutdown();
    assert!(request.join().unwrap().is_err());
    assert!(!service.worker_running().unwrap());
    let final_state = service.snapshot().unwrap();
    assert!(!final_state.state.enabled);
    assert!(final_state.error.is_none());
    assert_eq!(final_state.active_subscriptions, 0);
    assert!(service.set_enabled(true).is_err());
}
