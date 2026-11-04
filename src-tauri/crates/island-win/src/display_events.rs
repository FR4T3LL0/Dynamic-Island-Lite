//! Independent, event-driven display/foreground observer; no periodic idle polling.
use island_core::{
    display::{Foreground, Monitor},
    error::{AppError, AppResult},
};
use std::{
    cell::{Cell, RefCell},
    sync::{mpsc, Arc, OnceLock},
    thread::JoinHandle,
};
use windows::{
    core::w,
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
            WindowsAndMessaging::*,
        },
    },
};
const REFRESH: u32 = WM_APP + 71;
const FOREGROUND: u32 = WM_APP + 72;
const DISPLAY_TIMER: usize = 71;
const FOREGROUND_TIMER: usize = 72;

#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub monitors: Vec<Monitor>,
    pub fullscreen_monitors: Vec<String>,
    pub foreground: Option<Foreground>,
    pub error: Option<AppError>,
}
type Changed = Arc<dyn Fn(Observation, bool) + Send + Sync>;
struct Remembered {
    hwnd: HWND,
    pid: u32,
    watch: crate::process::ProcessWatch,
}
struct Context {
    monitors: Vec<Monitor>,
    last: Option<Observation>,
    remembered: Option<Remembered>,
    filter: Option<u32>,
    changed: Changed,
}
thread_local! { static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) }; }
thread_local! { static EVENT_SINK: Cell<isize> = const { Cell::new(0) }; }
thread_local! { static REMEMBERED_EVENT_WINDOW: Cell<isize> = const { Cell::new(0) }; }
thread_local! { static FOREGROUND_QUEUED: Cell<bool> = const { Cell::new(false) }; }
thread_local! { static FOREGROUND_ARMED: Cell<bool> = const { Cell::new(false) }; }

impl Context {
    fn observe(&mut self, refresh: bool) {
        let mut error = None;
        if refresh {
            match crate::display::monitors() {
                Ok(monitors) => self.monitors = monitors,
                Err(failure) => error = Some(failure),
            }
        }
        let mut foreground = unsafe { GetForegroundWindow() };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(foreground, Some(&mut pid));
        }
        if pid == std::process::id() {
            foreground = HWND::default();
            if let Some(previous) = &self.remembered {
                let mut current = 0;
                unsafe {
                    GetWindowThreadProcessId(previous.hwnd, Some(&mut current));
                }
                if current == previous.pid && !previous.watch.has_exited() {
                    foreground = previous.hwnd;
                }
            }
        } else if !foreground.is_invalid() && self.filter.is_none_or(|allowed| allowed == pid) {
            if self.remembered.as_ref().is_none_or(|previous| {
                previous.hwnd != foreground || previous.pid != pid || previous.watch.has_exited()
            }) {
                self.remembered = crate::process::ProcessWatch::open(pid, None)
                    .ok()
                    .map(|watch| Remembered {
                        hwnd: foreground,
                        pid,
                        watch,
                    });
            }
        } else {
            foreground = HWND::default();
            self.remembered = None;
        }
        REMEMBERED_EVENT_WINDOW.with(|target| {
            target.set(
                self.remembered
                    .as_ref()
                    .map_or(0, |window| window.hwnd.0 as isize),
            )
        });
        let assessment = if foreground.is_invalid() {
            None
        } else {
            crate::display::foreground(foreground, std::process::id()).ok()
        };
        let fullscreen_monitors = self
            .monitors
            .iter()
            .filter(|monitor| {
                assessment.is_some_and(|window| island_core::display::fullscreen(window, monitor))
            })
            .map(|m| m.id.clone())
            .collect();
        let next = Observation {
            monitors: self.monitors.clone(),
            fullscreen_monitors,
            foreground: assessment,
            error,
        };
        if refresh || self.last.as_ref() != Some(&next) {
            if self.filter.is_some()
                && (refresh
                    || self.last.as_ref().is_none_or(|previous| {
                        previous.fullscreen_monitors != next.fullscreen_monitors
                    }))
            {
                eprintln!(
                    "display-observer-test: {}",
                    serde_json::json!({"refresh":refresh,"foreground":foreground.0 as usize,"assessment":assessment.map(|window|serde_json::json!({"bounds":window.bounds,"visible":window.visible,"minimized":window.minimized,"maximized":window.maximized,"decorated":window.decorated,"excluded":window.excluded})),"fullscreen":next.fullscreen_monitors})
                );
            }
            self.last = Some(next.clone());
            (self.changed)(next, refresh);
        }
    }
}
fn observe(refresh: bool) {
    CONTEXT.with(|slot| {
        if let Ok(mut context) = slot.try_borrow_mut() {
            if let Some(context) = context.as_mut() {
                context.observe(refresh);
            }
        }
    });
}
unsafe extern "system" fn event_hook(
    _: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    if window.is_invalid() || object != OBJID_WINDOW.0 || child != 0 {
        return;
    }
    // Geometry notifications can arrive before the matching foreground change.
    // Queue a read for top-level windows without deciding their future relevance
    // inside this callback. Do not borrow the observation state reentrantly.
    let relevant = event == EVENT_SYSTEM_FOREGROUND
        || GetAncestor(window, GA_ROOT) == window
        || REMEMBERED_EVENT_WINDOW.with(|target| target.get() == window.0 as isize);
    let sink = EVENT_SINK.with(Cell::get);
    if relevant
        && sink != 0
        && !FOREGROUND_QUEUED.with(|queued| queued.replace(true))
        && PostMessageW(Some(HWND(sink as *mut _)), FOREGROUND, WPARAM(0), LPARAM(0)).is_err()
    {
        FOREGROUND_QUEUED.with(|queued| queued.set(false));
    }
}
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_DEVICECHANGE => {
            SetTimer(Some(window), DISPLAY_TIMER, 100, None);
            return LRESULT(0);
        }
        FOREGROUND => {
            FOREGROUND_QUEUED.with(|queued| queued.set(false));
            // Keep the first deadline during a burst; repeated notifications
            // must not postpone the observation indefinitely.
            if !FOREGROUND_ARMED.with(|armed| armed.replace(true))
                && SetTimer(Some(window), FOREGROUND_TIMER, 16, None) == 0
            {
                FOREGROUND_ARMED.with(|armed| armed.set(false));
            }
            return LRESULT(0);
        }
        WM_TIMER if wparam.0 == DISPLAY_TIMER || wparam.0 == FOREGROUND_TIMER => {
            let _ = KillTimer(Some(window), wparam.0);
            if wparam.0 == FOREGROUND_TIMER {
                FOREGROUND_ARMED.with(|armed| armed.set(false));
            }
            observe(wparam.0 == DISPLAY_TIMER);
            return LRESULT(0);
        }
        WM_DESTROY => {
            EVENT_SINK.with(|sink| sink.set(0));
            REMEMBERED_EVENT_WINDOW.with(|target| target.set(0));
            FOREGROUND_QUEUED.with(|queued| queued.set(false));
            FOREGROUND_ARMED.with(|armed| armed.set(false));
            PostQuitMessage(0);
            return LRESULT(0);
        }
        _ => {}
    }
    DefWindowProcW(window, message, wparam, lparam)
}
struct Hook(HWINEVENTHOOK);
impl Drop for Hook {
    fn drop(&mut self) {
        let _ = unsafe { UnhookWinEvent(self.0) };
    }
}
pub struct DisplayEvents {
    thread_id: u32,
    worker: Option<JoinHandle<()>>,
}
impl DisplayEvents {
    pub fn start(filter: Option<u32>, changed: Changed) -> AppResult<Self> {
        static CLASS: OnceLock<AppResult<()>> = OnceLock::new();
        CLASS
            .get_or_init(|| {
                let instance = unsafe { GetModuleHandleW(None) }
                    .map_err(|e| AppError::new("window_error", e.to_string()))?;
                let class = WNDCLASSW {
                    hInstance: HINSTANCE(instance.0),
                    lpfnWndProc: Some(window_proc),
                    lpszClassName: w!("DynamicIsland.DisplayEvents"),
                    ..Default::default()
                };
                if unsafe { RegisterClassW(&class) } == 0 {
                    return Err(AppError::new(
                        "window_error",
                        "Die Bildschirmüberwachung konnte nicht registriert werden.",
                    ));
                }
                Ok(())
            })
            .clone()?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("island-display-events".into())
            .spawn(move || {
                let result = (|| -> AppResult<()> {
                    let monitors = crate::display::monitors()?;
                    let instance = unsafe { GetModuleHandleW(None) }
                        .map_err(|e| AppError::new("window_error", e.to_string()))?;
                    let window = unsafe {
                        CreateWindowExW(
                            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                            w!("DynamicIsland.DisplayEvents"),
                            w!(""),
                            WS_POPUP,
                            0,
                            0,
                            1,
                            1,
                            None,
                            None,
                            Some(HINSTANCE(instance.0)),
                            None,
                        )
                    }
                    .map_err(|e| AppError::new("window_error", e.to_string()))?;
                    CONTEXT.with_borrow_mut(|slot| {
                        *slot = Some(Context {
                            monitors,
                            last: None,
                            remembered: None,
                            filter,
                            changed,
                        })
                    });
                    EVENT_SINK.with(|sink| sink.set(window.0 as isize));
                    let mut hooks = Vec::new();
                    for (first, last) in [
                        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
                        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
                        (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
                        (EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE),
                    ] {
                        let hook = unsafe {
                            SetWinEventHook(
                                first,
                                last,
                                None,
                                Some(event_hook),
                                0,
                                0,
                                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                            )
                        };
                        if hook.is_invalid() {
                            unsafe {
                                let _ = DestroyWindow(window);
                            };
                            return Err(AppError::new(
                                "window_error",
                                "Die Vordergrundüberwachung konnte nicht registriert werden.",
                            ));
                        }
                        hooks.push(Hook(hook));
                    }
                    observe(false);
                    let _ = sender.send(Ok(unsafe { GetCurrentThreadId() }));
                    let mut message = MSG::default();
                    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                        if message.hwnd.is_invalid() && message.message == REFRESH {
                            observe(true);
                            continue;
                        }
                        unsafe {
                            let _ = TranslateMessage(&message);
                            DispatchMessageW(&message);
                        }
                    }
                    drop(hooks);
                    unsafe {
                        let _ = KillTimer(Some(window), DISPLAY_TIMER);
                        let _ = KillTimer(Some(window), FOREGROUND_TIMER);
                        let _ = DestroyWindow(window);
                    }
                    CONTEXT.with_borrow_mut(|slot| *slot = None);
                    Ok(())
                })();
                if let Err(error) = result {
                    CONTEXT.with_borrow_mut(|slot| *slot = None);
                    let _ = sender.send(Err(error));
                }
            })
            .map_err(AppError::from)?;
        match receiver.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                worker: Some(worker),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err(AppError::new(
                    "window_error",
                    "Die Bildschirmüberwachung wurde beendet.",
                ))
            }
        }
    }
    pub fn refresh(&self) -> AppResult<()> {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            unsafe { PostThreadMessageW(self.thread_id, REFRESH, WPARAM(0), LPARAM(0)) }
                .map_err(|e| AppError::new("window_error", e.to_string()))?;
            return Ok(());
        }
        Err(AppError::new(
            "window_error",
            "Die Bildschirmüberwachung läuft nicht.",
        ))
    }
    pub fn stop(&mut self) -> AppResult<()> {
        if let Some(worker) = self.worker.take() {
            if !worker.is_finished()
                && unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
                    .is_err()
            {
                self.worker = Some(worker);
                return Err(AppError::new(
                    "window_error",
                    "Die Bildschirmüberwachung konnte nicht beendet werden.",
                ));
            }
            worker.join().map_err(|_| {
                AppError::new(
                    "window_error",
                    "Die Bildschirmüberwachung wurde unerwartet beendet.",
                )
            })?;
        }
        Ok(())
    }
}
impl Drop for DisplayEvents {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use windows::core::BOOL;
    unsafe extern "system" fn count_observers(window: HWND, data: LPARAM) -> BOOL {
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        let mut class = [0u16; 128];
        let length = GetClassNameW(window, &mut class).max(0) as usize;
        if pid == std::process::id()
            && String::from_utf16_lossy(&class[..length]) == "DynamicIsland.DisplayEvents"
        {
            assert!(!IsWindowVisible(window).as_bool());
            *(data.0 as *mut usize) += 1;
        }
        true.into()
    }
    fn observer_count() -> usize {
        let mut count = 0usize;
        unsafe {
            EnumWindows(
                Some(count_observers),
                LPARAM(&mut count as *mut usize as isize),
            )
        }
        .unwrap();
        count
    }
    unsafe extern "system" fn send_display_change(window: HWND, _: LPARAM) -> BOOL {
        let mut class = [0u16; 128];
        let length = GetClassNameW(window, &mut class).max(0) as usize;
        if String::from_utf16_lossy(&class[..length]) == "DynamicIsland.DisplayEvents" {
            PostMessageW(Some(window), WM_DISPLAYCHANGE, WPARAM(0), LPARAM(0)).unwrap();
        }
        true.into()
    }
    #[test]
    fn event_queue_accepts_root_geometry_before_activation() {
        assert!(queued_event(EVENT_OBJECT_LOCATIONCHANGE, false));
    }
    #[test]
    fn event_queue_accepts_reentrant_foreground_notification() {
        assert!(queued_event(EVENT_SYSTEM_FOREGROUND, true));
    }
    fn queued_event(event: u32, borrowed: bool) -> bool {
        let window = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        assert_ne!(unsafe { GetForegroundWindow() }, window);
        CONTEXT.with_borrow_mut(|slot| {
            *slot = Some(Context {
                monitors: vec![],
                last: None,
                remembered: None,
                filter: None,
                changed: Arc::new(|_, _| {}),
            })
        });
        EVENT_SINK.with(|sink| sink.set(window.0 as isize));
        let queued = CONTEXT.with(|slot| {
            let _borrow = borrowed.then(|| slot.borrow_mut());
            unsafe {
                event_hook(
                    HWINEVENTHOOK::default(),
                    event,
                    window,
                    OBJID_WINDOW.0,
                    0,
                    0,
                    0,
                )
            };
            let mut message = MSG::default();
            unsafe {
                PeekMessageW(
                    &mut message,
                    Some(window),
                    FOREGROUND,
                    FOREGROUND,
                    PM_REMOVE,
                )
            }
            .as_bool()
        });
        CONTEXT.with_borrow_mut(|slot| *slot = None);
        EVENT_SINK.with(|sink| sink.set(0));
        REMEMBERED_EVENT_WINDOW.with(|target| target.set(0));
        FOREGROUND_QUEUED.with(|queued| queued.set(false));
        FOREGROUND_ARMED.with(|armed| armed.set(false));
        unsafe { DestroyWindow(window) }.unwrap();
        queued
    }
    #[test]
    fn ten_start_refresh_stop_cycles_remove_hidden_windows_and_hooks() {
        let baseline = observer_count();
        for cycle in 0..10 {
            let (sender, receiver) = mpsc::channel();
            let mut events = DisplayEvents::start(
                Some(u32::MAX),
                Arc::new(move |value, refresh| {
                    let _ = sender.send((value, refresh));
                }),
            )
            .unwrap();
            let (initial, refresh) = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(!refresh);
            assert!(!initial.monitors.is_empty());
            assert!(initial.fullscreen_monitors.is_empty());
            assert_eq!(observer_count(), baseline + 1);
            events.refresh().unwrap();
            let (refreshed, refresh) = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(refresh);
            assert_eq!(initial, refreshed);
            if cycle == 0 {
                // A notification to our hidden top-level window exercises the
                // real broadcast handler without changing the user's topology.
                unsafe {
                    EnumThreadWindows(events.thread_id, Some(send_display_change), LPARAM(0))
                }
                .unwrap();
                let (notified, refresh) = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
                assert!(refresh);
                assert_eq!(notified, initial);
                assert!(matches!(
                    receiver.recv_timeout(Duration::from_millis(150)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ));
            }
            events.stop().unwrap();
            events.stop().unwrap();
            assert!(events.refresh().is_err());
            assert_eq!(observer_count(), baseline);
            assert!(matches!(
                receiver.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Disconnected)
            ));
        }
    }
}
