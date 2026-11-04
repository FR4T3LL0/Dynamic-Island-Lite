//! RegisterHotKey belongs to this worker's message queue; no low-level hook.
use island_core::{
    error::{AppError, AppResult},
    shortcut::Shortcut,
};
use serde::Serialize;
use std::{
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT,
        },
        WindowsAndMessaging::*,
    },
};
const REQUEST: u32 = WM_APP + 81;
const IDS: [i32; 2] = [0x4801, 0x4802];
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub registered: Option<String>,
    pub error: Option<AppError>,
}
enum Operation {
    Prepare(Shortcut),
    Finish(bool),
    Stop,
}
struct Request {
    operation: Operation,
    reply: mpsc::SyncSender<AppResult<()>>,
}
struct Connection {
    sender: mpsc::Sender<Request>,
    thread: u32,
    join: Option<JoinHandle<()>>,
}
pub struct Hotkey {
    connection: Mutex<Option<Connection>>,
    status: Arc<Mutex<Status>>,
}
fn failure(message: &str) -> AppError {
    AppError::new("io_error", message)
}
fn register(key: &Shortcut, id: i32) -> AppResult<()> {
    unsafe { RegisterHotKey(None, id, HOT_KEY_MODIFIERS(key.modifiers | MOD_NOREPEAT.0), key.key) }
        .map_err(|e| {
            let conflict=e.code().0 as u32 == 0x80070581;
            AppError::new(if conflict {"conflict"} else {"access_denied"},
                format!("{} konnte nicht registriert werden. {} Die Insel bleibt über das Tray erreichbar.", key.label,
                    if conflict {"Das Kürzel ist bereits belegt. Bitte ein anderes wählen."} else {"Windows hat die Registrierung verweigert. Bitte ein anderes Kürzel wählen oder die Registrierung erneut versuchen."}))
        })
}
fn call(connection: &Connection, operation: Operation) -> AppResult<()> {
    let (reply, response) = mpsc::sync_channel(1);
    connection
        .sender
        .send(Request { operation, reply })
        .map_err(|_| failure("Der Tastenkürzeldienst wurde beendet."))?;
    unsafe { PostThreadMessageW(connection.thread, REQUEST, WPARAM(0), LPARAM(0)) }
        .map_err(|e| failure(&e.to_string()))?;
    response.recv_timeout(Duration::from_secs(5)).map_err(|_| {
        failure(
            "Der Tastenkürzeldienst antwortet nicht. Bitte die Insel über das Tray neu starten.",
        )
    })?
}
impl Hotkey {
    pub fn start(value: &str, activated: Arc<dyn Fn() + Send + Sync>) -> AppResult<Self> {
        let key = Shortcut::parse(value)?;
        let status = Arc::new(Mutex::new(Status::default()));
        let shared = status.clone();
        let (sender, requests) = mpsc::channel::<Request>();
        let (ready, startup) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("island-hotkey".into())
            .spawn(move || {
                let mut message = MSG::default();
                unsafe {
                    let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                }
                let mut current = match register(&key, IDS[0]) {
                    Ok(()) => {
                        *shared.lock().unwrap() = Status {
                            registered: Some(key.label.clone()),
                            error: None,
                        };
                        Some((IDS[0], key))
                    }
                    Err(error) => {
                        shared.lock().unwrap().error = Some(error);
                        None
                    }
                };
                let mut pending: Option<(i32, Shortcut)> = None;
                if ready.send(unsafe { GetCurrentThreadId() }).is_err() {
                    for id in IDS {
                        unsafe {
                            let _ = UnregisterHotKey(None, id);
                        }
                    }
                    return;
                }
                let mut stopped = false;
                while !stopped && unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                    if message.message == WM_HOTKEY
                        && current.as_ref().is_some_and(|(id, key)| {
                            *id as usize == message.wParam.0
                                && (message.lParam.0 as u32 >> 16) == key.key
                                && (message.lParam.0 as u32 & 0x7) == key.modifiers
                        })
                    {
                        activated();
                    }
                    if message.message != REQUEST {
                        continue;
                    }
                    while let Ok(request) = requests.try_recv() {
                        let result = (|| -> AppResult<()> {
                            match request.operation {
                                Operation::Prepare(next) => {
                                    if let Some((id, _)) = pending.take() {
                                        unsafe { UnregisterHotKey(None, id) }
                                            .map_err(|e| failure(&e.to_string()))?;
                                    }
                                    if current.as_ref().is_some_and(|(_, key)| *key == next) {
                                        return Ok(());
                                    }
                                    let id =
                                        if current.as_ref().is_some_and(|(id, _)| *id == IDS[0]) {
                                            IDS[1]
                                        } else {
                                            IDS[0]
                                        };
                                    register(&next, id)?;
                                    pending = Some((id, next));
                                }
                                Operation::Finish(commit) => {
                                    if let Some(next) = pending.take() {
                                        if commit {
                                            if let Some((old, _)) = current.take() {
                                                unsafe {
                                                    let _ = UnregisterHotKey(None, old);
                                                }
                                            }
                                            *shared.lock().unwrap() = Status {
                                                registered: Some(next.1.label.clone()),
                                                error: None,
                                            };
                                            current = Some(next);
                                        } else {
                                            unsafe { UnregisterHotKey(None, next.0) }
                                                .map_err(|e| failure(&e.to_string()))?;
                                        }
                                    }
                                }
                                Operation::Stop => {
                                    stopped = true;
                                }
                            }
                            Ok(())
                        })();
                        let _ = request.reply.send(result);
                        if stopped {
                            break;
                        }
                    }
                }
                for id in IDS {
                    unsafe {
                        let _ = UnregisterHotKey(None, id);
                    }
                }
                *shared.lock().unwrap() = Status::default();
            })
            .map_err(|e| failure(&e.to_string()))?;
        let thread = match startup.recv_timeout(Duration::from_secs(5)) {
            Ok(thread) => thread,
            Err(_) => {
                drop(startup);
                let _ = join.join();
                return Err(failure(
                    "Der Tastenkürzeldienst konnte nicht gestartet werden.",
                ));
            }
        };
        Ok(Self {
            connection: Mutex::new(Some(Connection {
                sender,
                thread,
                join: Some(join),
            })),
            status,
        })
    }
    pub fn snapshot(&self) -> AppResult<Status> {
        self.status
            .lock()
            .map(|s| s.clone())
            .map_err(|_| failure("Der Tastenkürzelstatus ist gesperrt."))
    }
    /// Keep the old registration until durable storage succeeds. Roll back only
    /// the newly reserved registration if storage fails, without a rebind race.
    pub fn change<T>(&self, value: &str, persist: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
        let key = Shortcut::parse(value)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| failure("Der Tastenkürzeldienst ist gesperrt."))?;
        let connection = connection
            .as_ref()
            .ok_or_else(|| failure("Der Tastenkürzeldienst wurde beendet."))?;
        call(connection, Operation::Prepare(key))?;
        match persist() {
            Ok(value) => {
                call(connection, Operation::Finish(true))?;
                Ok(value)
            }
            Err(error) => {
                call(connection, Operation::Finish(false))?;
                Err(error)
            }
        }
    }
    pub fn stop(&self) {
        if let Ok(mut slot) = self.connection.lock() {
            if let Some(mut connection) = slot.take() {
                let _ = call(&connection, Operation::Stop);
                if let Some(join) = connection.join.take() {
                    let _ = join.join();
                }
            }
        }
    }
}
impl Drop for Hotkey {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collision_rebind_storage_rollback_and_ten_lifecycles() {
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let blocker = Hotkey::start("Ctrl+Alt+Shift+F23", callback.clone()).unwrap();
        assert!(blocker.snapshot().unwrap().registered.is_some());
        let service = Hotkey::start("Ctrl+Alt+Shift+F23", callback.clone()).unwrap();
        assert!(service.snapshot().unwrap().registered.is_none());
        assert_eq!(service.snapshot().unwrap().error.unwrap().code, "conflict");
        service.change("Ctrl+Alt+Shift+F24", || Ok(())).unwrap();
        assert_eq!(
            service.snapshot().unwrap().registered.as_deref(),
            Some("Ctrl+Alt+Shift+F24")
        );
        let mut stored = false;
        assert!(service
            .change("Ctrl+Alt+Shift+F23", || {
                stored = true;
                Ok(())
            })
            .is_err());
        assert!(!stored);
        blocker.stop();
        assert!(service
            .change("Ctrl+Alt+Shift+F23", || Err::<(), _>(failure(
                "Speicherfehler"
            )))
            .is_err());
        assert_eq!(
            service.snapshot().unwrap().registered.as_deref(),
            Some("Ctrl+Alt+Shift+F24")
        );
        for _ in 0..10 {
            let probe = Hotkey::start("Ctrl+Alt+Shift+F23", callback.clone()).unwrap();
            assert!(probe.snapshot().unwrap().registered.is_some());
            probe.stop();
            probe.stop();
        }
        service.change("Ctrl+Alt+Shift+F23", || Ok(())).unwrap();
        let released = Hotkey::start("Ctrl+Alt+Shift+F24", callback.clone()).unwrap();
        assert!(released.snapshot().unwrap().registered.is_some());
        service.stop();
        let released = Hotkey::start("Ctrl+Alt+Shift+F23", callback).unwrap();
        assert!(released.snapshot().unwrap().registered.is_some());
    }
}
