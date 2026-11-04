//! Per-profile/session single instance. The open file denies other writers;
//! Windows releases that ownership even after an abrupt process termination.
use island_core::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::{self, Write},
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
    sync::{mpsc, Arc, Mutex, OnceLock},
    thread::JoinHandle,
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR, PWSTR},
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            NtCreateFile, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF,
            FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
            NTCREATEFILE_CREATE_OPTIONS,
        },
    },
    Win32::{
        Foundation::{
            RtlNtStatusToDosError, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT,
            OBJ_CASE_INSENSITIVE, UNICODE_STRING, WPARAM,
        },
        Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ACCESS_RIGHTS,
            FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
            FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, SYNCHRONIZE,
        },
        System::{
            LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId, IO::IO_STATUS_BLOCK,
        },
        UI::WindowsAndMessaging::*,
    },
};
const ACTIVATE: u32 = WM_APP + 90;
const READY: u32 = WM_APP + 91;
const SHUTDOWN: u32 = WM_APP + 92;
#[derive(Serialize, Deserialize)]
struct Owner {
    pid: u32,
    started: u64,
    session: u32,
    hwnd: u64,
    token: usize,
}
struct Context {
    token: usize,
    ready: bool,
    pending: bool,
    shutdown_pending: bool,
    activate: Arc<dyn Fn() + Send + Sync>,
    shutdown: Arc<dyn Fn() + Send + Sync>,
}
thread_local! {static CONTEXT:RefCell<Option<Context>>=const {RefCell::new(None)};}
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == ACTIVATE || msg == SHUTDOWN {
        CONTEXT.with_borrow_mut(|slot| {
            if let Some(context) = slot {
                if context.token == wparam.0 {
                    if msg == SHUTDOWN {
                        context.shutdown_pending = true;
                        if context.ready {
                            context.shutdown_pending = false;
                            (context.shutdown)();
                        }
                        return;
                    }
                    context.pending = true;
                    if context.ready {
                        context.pending = false;
                        (context.activate)();
                    }
                }
            }
        });
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
struct Worker {
    thread: u32,
    join: JoinHandle<()>,
}
pub struct Instance {
    file: Mutex<Option<File>>,
    maintenance: Mutex<Option<File>>,
    worker: Mutex<Option<Worker>>,
}
pub enum Start {
    Primary(Instance),
    Forwarded,
}
fn error(message: impl Into<String>) -> AppError {
    AppError::new("conflict", message)
}
/// Outside the optional user-data removal directory; shared by running apps,
/// exclusively held by the installer until its file operations have finished.
pub fn maintenance_path(directory: &Path, session: u32) -> AppResult<std::path::PathBuf> {
    let parent = directory
        .parent()
        .ok_or_else(|| error("Der Profilordner hat kein übergeordnetes Verzeichnis."))?;
    let name = directory
        .file_name()
        .ok_or_else(|| error("Der Profilordner hat keinen Namen."))?;
    let mut file = std::ffi::OsString::from(".");
    file.push(name);
    file.push(format!("-maintenance-{session}.lock"));
    Ok(parent.join(file))
}
fn open_roaming_base(directory: &Path, create: bool) -> io::Result<File> {
    let parent = directory
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Profil ohne Elternordner"))?;
    if create {
        std::fs::create_dir_all(parent)?;
    }
    // The configured roaming base may be redirected by Windows. Pin its resolved directory.
    OpenOptions::new()
        .access_mode((FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE).0)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(parent)
}

fn open_child(
    parent: &File,
    name: &OsStr,
    access: FILE_ACCESS_RIGHTS,
    share: FILE_SHARE_MODE,
    kind: NTCREATEFILE_CREATE_OPTIONS,
    create: bool,
) -> io::Result<File> {
    let mut name_utf16: Vec<u16> = name.encode_wide().collect();
    let invalid_name = || io::Error::new(io::ErrorKind::InvalidInput, "Ungültiger Profilname");
    let forbidden = [0, u16::from(b'/'), u16::from(b'\\'), u16::from(b':')];
    if name.is_empty()
        || name == "."
        || name == ".."
        || name_utf16.iter().any(|unit| forbidden.contains(unit))
    {
        return Err(invalid_name());
    }
    let length = u16::try_from(name_utf16.len() * 2).map_err(|_| invalid_name())?;
    let name = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: PWSTR(name_utf16.as_mut_ptr()),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: HANDLE(parent.as_raw_handle()),
        ObjectName: &name,
        Attributes: OBJ_CASE_INSENSITIVE,
        ..Default::default()
    };
    let mut handle = HANDLE::default();
    let mut status_block = IO_STATUS_BLOCK::default();
    // A single name relative to an open directory prevents ancestor substitutions.
    // Open the child itself and inspect that handle before reading or writing it.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access | SYNCHRONIZE,
            &attributes,
            &mut status_block,
            None,
            FILE_ATTRIBUTE_NORMAL,
            share,
            if create { FILE_OPEN_IF } else { FILE_OPEN },
            kind | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            None,
            0,
        )
    };
    if status.0 < 0 {
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(status) } as i32,
        ));
    }
    // NtCreateFile transferred ownership on success. File closes even on validation failure.
    let file = unsafe { File::from_raw_handle(handle.0) };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(handle, &mut info) }.map_err(io::Error::other)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
        || (kind == FILE_NON_DIRECTORY_FILE && info.nNumberOfLinks != 1)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Der Profilordner oder die Sperrdatei ist eine unzulässige Dateiverknüpfung.",
        ));
    }
    Ok(file)
}

fn open_profile(base: &File, directory: &Path, create: bool) -> io::Result<File> {
    let name = directory
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Profil ohne Ordnernamen"))?;
    open_child(
        base,
        name,
        FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        FILE_DIRECTORY_FILE,
        create,
    )
}

fn startup_guard(base: &File, directory: &Path, session: u32) -> AppResult<File> {
    let path = maintenance_path(directory, session)?;
    open_child(
        base,
        path.file_name().expect("maintenance file name"),
        FILE_GENERIC_READ | FILE_GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        FILE_NON_DIRECTORY_FILE,
        true,
    )
    .map_err(|e| {
        error(format!(
            "Installation oder Wartung sperrt den Start. Bitte danach erneut starten: {e}"
        ))
    })
}
/// No termination API: request the normal exit path and wait on the same held
/// process handle. Missing, foreign, stale or unresponsive owners cannot ACK.
pub fn shutdown_existing(
    directory: &Path,
    expected_executable: &Path,
    timeout: Duration,
) -> AppResult<bool> {
    if !expected_executable.is_absolute() {
        return Err(error("Für die Wartung fehlt ein absoluter Programmpfad."));
    }
    let session = crate::window::current_session()?;
    let profile = match open_roaming_base(directory, false)
        .and_then(|base| open_profile(&base, directory, false))
    {
        Ok(profile) => profile,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    let lock_name = format!("instance-{session}.lock");
    let name = OsStr::new(&lock_name);
    let deadline = Instant::now() + timeout;
    loop {
        match open_child(
            &profile,
            name,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_READ,
            FILE_NON_DIRECTORY_FILE,
            false,
        ) {
            Ok(_) => return Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) if e.raw_os_error() == Some(32) => {}
            Err(e) => return Err(e.into()),
        }
        let owner = (|| -> AppResult<_> {
            let file = open_child(
                &profile,
                name,
                FILE_GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                FILE_NON_DIRECTORY_FILE,
                false,
            )?;
            let owner: Owner = serde_json::from_reader(std::io::Read::take(file, 4096))
                .map_err(|e| error(e.to_string()))?;
            if owner.session != session {
                return Err(error("Die Inselinstanz gehört zu einer anderen Sitzung."));
            }
            let watch = crate::process::ProcessWatch::open(owner.pid, Some(owner.started))?;
            if watch.executable_path()?.canonicalize()? != expected_executable.canonicalize()? {
                return Err(error(
                    "Die laufende Insel gehört zu einem anderen Installationspfad.",
                ));
            }
            let hwnd = HWND(
                usize::try_from(owner.hwnd)
                    .map_err(|_| error("Ungültige Instanzfensterkennung."))?
                    as *mut _,
            );
            let mut pid = 0;
            unsafe {
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
            }
            let mut class = [0u16; 80];
            let length = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
            if watch.has_exited()
                || pid != owner.pid
                || String::from_utf16_lossy(&class[..length]) != "DynamicIslandLite.Instance"
            {
                return Err(error("Die laufende Inselidentität ist nicht mehr gültig."));
            }
            Ok((owner, watch, hwnd))
        })();
        match owner {
            Ok((owner, watch, hwnd)) => {
                unsafe { PostMessageW(Some(hwnd), SHUTDOWN, WPARAM(owner.token), LPARAM(0)) }
                    .map_err(|e| error(e.to_string()))?;
                while !watch.has_exited() {
                    if Instant::now() >= deadline {
                        return Err(error("Die Insel konnte ihre Fenster nicht rechtzeitig wiederherstellen und beenden. Installation wurde angehalten."));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                return Ok(true);
            }
            Err(e) => {
                if Instant::now() >= deadline {
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
impl Instance {
    pub fn start(directory: &Path, activate: Arc<dyn Fn() + Send + Sync>) -> AppResult<Start> {
        Self::start_with_shutdown(directory, activate, Arc::new(|| {}))
    }
    pub fn start_with_shutdown(
        directory: &Path,
        activate: Arc<dyn Fn() + Send + Sync>,
        shutdown: Arc<dyn Fn() + Send + Sync>,
    ) -> AppResult<Start> {
        let base = open_roaming_base(directory, true)?;
        let session = crate::window::current_session()?;
        let maintenance = startup_guard(&base, directory, session)?;
        let profile = open_profile(&base, directory, true)?;
        let lock_name = format!("instance-{session}.lock");
        let name = OsStr::new(&lock_name);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut file = loop {
            match open_child(
                &profile,
                name,
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                FILE_SHARE_READ,
                FILE_NON_DIRECTORY_FILE,
                true,
            ) {
                Ok(file) => break file,
                Err(e) if e.raw_os_error() == Some(32) => {
                    let owner = (|| -> Option<Owner> {
                        let file = open_child(
                            &profile,
                            name,
                            FILE_GENERIC_READ,
                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                            FILE_NON_DIRECTORY_FILE,
                            false,
                        )
                        .ok()?;
                        let owner: Owner =
                            serde_json::from_reader(std::io::Read::take(file, 4096)).ok()?;
                        if owner.session != session {
                            return None;
                        }
                        let watch =
                            crate::process::ProcessWatch::open(owner.pid, Some(owner.started))
                                .ok()?;
                        if watch.has_exited() {
                            return None;
                        }
                        let hwnd = HWND(usize::try_from(owner.hwnd).ok()? as *mut _);
                        let mut pid = 0;
                        unsafe {
                            GetWindowThreadProcessId(hwnd, Some(&mut pid));
                        }
                        let mut class = [0u16; 80];
                        let length = unsafe { GetClassNameW(hwnd, &mut class) } as usize;
                        if pid != owner.pid
                            || String::from_utf16_lossy(&class[..length])
                                != "DynamicIslandLite.Instance"
                        {
                            return None;
                        }
                        Some(owner)
                    })();
                    if let Some(owner) = owner {
                        unsafe {
                            // A user-initiated second launch may transfer its foreground grant.
                            let _ = AllowSetForegroundWindow(owner.pid);
                            PostMessageW(
                                Some(HWND(owner.hwnd as usize as *mut _)),
                                ACTIVATE,
                                WPARAM(owner.token),
                                LPARAM(0),
                            )
                        }
                        .map_err(|e| error(e.to_string()))?;
                        return Ok(Start::Forwarded);
                    }
                    if Instant::now() >= deadline {
                        return Err(error("Eine Inselinstanz läuft bereits, antwortet aber noch nicht. Bitte über das Tray öffnen oder nach ihrem Beenden erneut starten."));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(e.into()),
            }
        };
        static CLASS: OnceLock<AppResult<()>> = OnceLock::new();
        CLASS
            .get_or_init(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(window_proc),
                    hInstance: HINSTANCE(
                        unsafe { GetModuleHandleW(None) }
                            .map_err(|e| error(e.to_string()))?
                            .0,
                    ),
                    lpszClassName: w!("DynamicIslandLite.Instance"),
                    ..Default::default()
                };
                if unsafe { RegisterClassW(&class) } == 0 {
                    Err(error(
                        "Die Instanzkommunikation konnte nicht registriert werden.",
                    ))
                } else {
                    Ok(())
                }
            })
            .clone()?;
        let token = usize::from_str_radix(&crate::process::random_id()?[..16], 16)
            .map_err(|e| error(e.to_string()))?;
        let (ready, startup) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("island-instance".into())
            .spawn(move || {
                let result = (|| -> AppResult<()> {
                    let instance = HINSTANCE(
                        unsafe { GetModuleHandleW(None) }
                            .map_err(|e| error(e.to_string()))?
                            .0,
                    );
                    let hwnd = unsafe {
                        CreateWindowExW(
                            WINDOW_EX_STYLE(0),
                            w!("DynamicIslandLite.Instance"),
                            PCWSTR::null(),
                            WINDOW_STYLE(0),
                            0,
                            0,
                            0,
                            0,
                            Some(HWND_MESSAGE),
                            None,
                            Some(instance),
                            None,
                        )
                    }
                    .map_err(|e| error(e.to_string()))?;
                    CONTEXT.with_borrow_mut(|slot| {
                        *slot = Some(Context {
                            token,
                            ready: false,
                            pending: false,
                            shutdown_pending: false,
                            activate,
                            shutdown,
                        })
                    });
                    if ready
                        .send(Ok((hwnd.0 as usize as u64, unsafe {
                            GetCurrentThreadId()
                        })))
                        .is_err()
                    {
                        unsafe {
                            let _ = DestroyWindow(hwnd);
                        }
                        return Ok(());
                    }
                    let mut message = MSG::default();
                    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                        if message.hwnd.is_invalid() && message.message == READY {
                            CONTEXT.with_borrow_mut(|slot| {
                                if let Some(context) = slot {
                                    context.ready = true;
                                    if context.shutdown_pending {
                                        context.shutdown_pending = false;
                                        (context.shutdown)();
                                    } else if context.pending {
                                        context.pending = false;
                                        (context.activate)();
                                    }
                                }
                            });
                        } else {
                            unsafe {
                                let _ = TranslateMessage(&message);
                                DispatchMessageW(&message);
                            }
                        }
                    }
                    CONTEXT.with_borrow_mut(|slot| *slot = None);
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    let _ = ready.send(Err(e));
                }
            })?;
        let (hwnd, thread) = match startup.recv() {
            Ok(Ok(value)) => value,
            Ok(Err(e)) => {
                let _ = join.join();
                return Err(e);
            }
            Err(_) => {
                let _ = join.join();
                return Err(error("Die Instanzkommunikation wurde unterbrochen."));
            }
        };
        let instance = Self {
            file: Mutex::new(None),
            maintenance: Mutex::new(Some(maintenance)),
            worker: Mutex::new(Some(Worker { thread, join })),
        };
        let owner = Owner {
            pid: std::process::id(),
            started: crate::process::ProcessWatch::open(std::process::id(), None)?.started_at,
            session,
            hwnd,
            token,
        };
        file.set_len(0)?;
        file.write_all(&serde_json::to_vec(&owner).map_err(|e| error(e.to_string()))?)?;
        file.sync_all()?;
        *instance.file.lock().unwrap() = Some(file);
        Ok(Start::Primary(instance))
    }
    pub fn ready(&self) -> AppResult<()> {
        let worker = self
            .worker
            .lock()
            .map_err(|_| error("Die Instanzkommunikation ist gesperrt."))?;
        let worker = worker
            .as_ref()
            .ok_or_else(|| error("Die Instanz wurde beendet."))?;
        unsafe { PostThreadMessageW(worker.thread, READY, WPARAM(0), LPARAM(0)) }
            .map_err(|e| error(e.to_string()))
    }
    pub fn stop(&self) {
        if let Ok(mut slot) = self.worker.lock() {
            if let Some(worker) = slot.take() {
                unsafe {
                    let _ = PostThreadMessageW(worker.thread, WM_QUIT, WPARAM(0), LPARAM(0));
                }
                let _ = worker.join.join();
            }
        }
        if let Ok(mut file) = self.file.lock() {
            file.take();
        }
        if let Ok(mut file) = self.maintenance.lock() {
            file.take();
        }
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_a_hard_link_without_truncating_its_target() {
        let root = std::env::temp_dir().join(format!(
            "island-instance-link-test-{}",
            crate::process::random_id().unwrap()
        ));
        let profile = root.join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        let target = root.join("keep.txt");
        std::fs::write(&target, b"do not truncate").unwrap();
        let session = crate::window::current_session().unwrap();
        let lock = profile.join(format!("instance-{session}.lock"));
        std::fs::hard_link(&target, &lock).unwrap();

        assert!(Instance::start(&profile, Arc::new(|| {})).is_err());
        assert!(shutdown_existing(
            &profile,
            &std::env::current_exe().unwrap(),
            Duration::from_millis(100),
        ).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"do not truncate");

        std::fs::remove_file(&lock).unwrap();
        let Start::Primary(primary) = Instance::start(&profile, Arc::new(|| {})).unwrap() else {
            panic!("Keine primäre Instanz");
        };
        primary.stop();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "benötigt die Windows-Berechtigung zum Anlegen symbolischer Links"]
    fn rejects_links_in_the_profile_but_accepts_a_redirected_roaming_base() {
        use std::os::windows::fs::{symlink_dir, symlink_file};

        let root = std::env::temp_dir().join(format!(
            "island-instance-reparse-test-{}",
            crate::process::random_id().unwrap()
        ));
        let profile = root.join("profile");
        let target_dir = root.join("other");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();
        let lock_name = format!("instance-{}.lock", crate::window::current_session().unwrap());
        let target = target_dir.join(&lock_name);
        std::fs::write(&target, b"keep").unwrap();

        let lock = profile.join(&lock_name);
        symlink_file(&target, &lock).unwrap();
        assert!(Instance::start(&profile, Arc::new(|| {})).is_err());
        std::fs::remove_file(&lock).unwrap();
        std::fs::remove_dir(&profile).unwrap();

        symlink_dir(&target_dir, &profile).unwrap();
        assert!(Instance::start(&profile, Arc::new(|| {})).is_err());
        assert!(shutdown_existing(
            &profile,
            &std::env::current_exe().unwrap(),
            Duration::from_millis(100),
        ).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"keep");
        std::fs::remove_dir(&profile).unwrap();

        let roaming = root.join("redirected-roaming");
        symlink_dir(&target_dir, &roaming).unwrap();
        let Start::Primary(primary) = Instance::start(&roaming.join("profile"), Arc::new(|| {})).unwrap() else {
            panic!("Keine primäre Instanz");
        };
        primary.stop();
        std::fs::remove_dir(&roaming).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn serializes_ownership_preserves_early_activation_and_releases_after_ten_cycles() {
        let path = std::env::temp_dir().join(format!(
            "island-instance-test-{}",
            crate::process::random_id().unwrap()
        ));
        let (sender, receiver) = mpsc::channel();
        for _ in 0..10 {
            let sender = sender.clone();
            let Start::Primary(primary) = Instance::start(
                &path,
                Arc::new(move || {
                    let _ = sender.send(());
                }),
            )
            .unwrap() else {
                panic!("Keine primäre Instanz");
            };
            assert!(matches!(
                Instance::start(&path, Arc::new(|| {})).unwrap(),
                Start::Forwarded
            ));
            assert!(receiver.recv_timeout(Duration::from_millis(30)).is_err());
            primary.ready().unwrap();
            receiver.recv_timeout(Duration::from_secs(1)).unwrap();
            assert!(matches!(
                Instance::start(&path, Arc::new(|| {})).unwrap(),
                Start::Forwarded
            ));
            receiver.recv_timeout(Duration::from_secs(1)).unwrap();
            primary.stop();
            primary.stop();
        }
        std::fs::remove_file(
            maintenance_path(&path, crate::window::current_session().unwrap()).unwrap(),
        )
        .unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn installer_exclusion_blocks_start_before_profile_creation_and_releases_cleanly() {
        let root = std::env::temp_dir().join(format!(
            "island-installer-test-{}",
            crate::process::random_id().unwrap()
        ));
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::create_dir(&root).unwrap();
        let profile = root.join("Profil mit Umlaut ü");
        let guard = maintenance_path(&profile, crate::window::current_session().unwrap()).unwrap();
        let exclusive = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(&guard)
            .unwrap();
        assert!(Instance::start(&profile, Arc::new(|| {})).is_err());
        assert!(!profile.exists());
        drop(exclusive);
        let Start::Primary(primary) = Instance::start(&profile, Arc::new(|| {})).unwrap() else {
            panic!("Kein neuer Start")
        };
        assert!(OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&guard)
            .is_err());
        assert!(matches!(
            Instance::start(&profile, Arc::new(|| {})).unwrap(),
            Start::Forwarded
        ));
        primary.stop();
        assert!(OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&guard)
            .is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn shutdown_requires_the_nonce_and_readiness_and_never_treats_a_callback_as_process_exit() {
        let root = std::env::temp_dir().join(format!(
            "island-installer-test-{}",
            crate::process::random_id().unwrap()
        ));
        assert!(root.starts_with(std::env::temp_dir()));
        let profile = root.join("profile");
        let (sender, receiver) = mpsc::channel();
        let Start::Primary(primary) = Instance::start_with_shutdown(
            &profile,
            Arc::new(|| {}),
            Arc::new(move || {
                let _ = sender.send(());
            }),
        )
        .unwrap() else {
            panic!("Keine eigene Instanz")
        };
        let owner: Owner = serde_json::from_reader(
            OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
                .open(profile.join(format!(
                    "instance-{}.lock",
                    crate::window::current_session().unwrap()
                )))
                .unwrap(),
        )
        .unwrap();
        let hwnd = HWND(owner.hwnd as usize as *mut _);
        unsafe { PostMessageW(Some(hwnd), SHUTDOWN, WPARAM(owner.token ^ 1), LPARAM(0)) }.unwrap();
        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
        unsafe { PostMessageW(Some(hwnd), SHUTDOWN, WPARAM(owner.token), LPARAM(0)) }.unwrap();
        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
        primary.ready().unwrap();
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(shutdown_existing(
            &profile,
            &std::env::current_exe().unwrap(),
            Duration::from_millis(100)
        )
        .is_err());
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        primary.stop();
        assert!(!shutdown_existing(
            &profile,
            &std::env::current_exe().unwrap(),
            Duration::from_millis(100)
        )
        .unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
}
