use island_core::error::{AppError, AppResult};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
use windows::Win32::{
    Foundation::HANDLE,
    System::{
        Power::{
            PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
            DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY,
        },
        WindowsProgramming::QueryUnbiasedInterruptTimePrecise,
    },
    UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    },
};
pub const SUSPENDED: u32 = 1;
pub const RESUMED: u32 = 2;
#[derive(Default)]
struct State {
    pending: AtomicU32,
    suspended: AtomicBool,
}
unsafe extern "system" fn notify(
    context: *const std::ffi::c_void,
    kind: u32,
    _: *const std::ffi::c_void,
) -> u32 {
    if context.is_null() {
        return 0;
    }
    let state = &*context.cast::<State>();
    if kind == PBT_APMSUSPEND {
        state.suspended.store(true, Ordering::Release);
        state.pending.fetch_or(SUSPENDED, Ordering::AcqRel);
    }
    if (kind == PBT_APMRESUMEAUTOMATIC || kind == PBT_APMRESUMESUSPEND)
        && state.suspended.swap(false, Ordering::AcqRel)
    {
        state.pending.fetch_or(RESUMED, Ordering::AcqRel);
    }
    0
}
pub struct PowerEvents {
    handle: Option<HPOWERNOTIFY>,
    state: Option<Box<State>>,
    parameters: Option<Box<DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS>>,
    registered: Arc<AtomicBool>,
}
impl PowerEvents {
    pub fn register() -> AppResult<Self> {
        Self::register_with_status(Arc::new(AtomicBool::new(false)))
    }
    pub(crate) fn register_with_status(registered: Arc<AtomicBool>) -> AppResult<Self> {
        if registered.load(Ordering::Acquire) {
            return Err(AppError::new(
                "power_error",
                "Die vorherige Standby-Abmeldung ist nicht bestätigt.",
            ));
        }
        let mut state = Box::<State>::default();
        let parameters = Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(notify),
            Context: (&mut *state as *mut State).cast(),
        });
        let mut handle = std::ptr::null_mut();
        let result = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                HANDLE(
                    (&*parameters as *const DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS)
                        .cast_mut()
                        .cast(),
                ),
                &mut handle,
            )
        };
        if result.0 != 0 {
            return Err(AppError::new(
                "power_error",
                format!(
                    "Standby-Ereignisse konnten nicht registriert werden (Windows {}).",
                    result.0
                ),
            ));
        }
        registered.store(true, Ordering::Release);
        Ok(Self {
            handle: Some(HPOWERNOTIFY(handle as isize)),
            state: Some(state),
            parameters: Some(parameters),
            registered,
        })
    }
    pub fn take(&self) -> u32 {
        self.state
            .as_ref()
            .map_or(0, |state| state.pending.swap(0, Ordering::AcqRel))
    }
    pub fn registered(&self) -> bool {
        self.handle.is_some()
    }
    pub fn close(&mut self) -> AppResult<()> {
        self.close_with(|handle| unsafe { PowerUnregisterSuspendResumeNotification(handle) }.0)
    }
    fn close_with(&mut self, remove: impl FnOnce(HPOWERNOTIFY) -> u32) -> AppResult<()> {
        let Some(handle) = self.handle else {
            return Ok(());
        };
        let error = remove(handle);
        if error != 0 {
            return Err(AppError::new(
                "power_error",
                format!("Standby-Ereignisse konnten nicht abgemeldet werden (Windows {error})."),
            ));
        }
        self.handle = None;
        self.state = None;
        self.parameters = None;
        self.registered.store(false, Ordering::Release);
        Ok(())
    }
}
impl Drop for PowerEvents {
    fn drop(&mut self) {
        if self.close().is_err() {
            if let Some(state) = self.state.take() {
                let _ = Box::leak(state);
            }
            if let Some(parameters) = self.parameters.take() {
                let _ = Box::leak(parameters);
            }
        }
    }
}
/// A heartbeat measures awake progress; system sleep must not look like a dead controller.
pub fn awake_ms() -> u64 {
    (unsafe { QueryUnbiasedInterruptTimePrecise() }) / 10_000
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_resume_notifications_do_not_restart_grace() {
        let state = State::default();
        let pointer = &state as *const State as *const std::ffi::c_void;
        unsafe {
            notify(pointer, PBT_APMSUSPEND, std::ptr::null());
            notify(pointer, PBT_APMRESUMEAUTOMATIC, std::ptr::null());
            notify(pointer, PBT_APMRESUMESUSPEND, std::ptr::null());
        }
        assert_eq!(state.pending.swap(0, Ordering::AcqRel), SUSPENDED | RESUMED);
        unsafe {
            notify(pointer, PBT_APMRESUMESUSPEND, std::ptr::null());
        }
        assert_eq!(state.pending.load(Ordering::Acquire), 0);
    }
    #[test]
    fn real_power_subscription_and_awake_clock_are_available() {
        let mut events = PowerEvents::register().unwrap();
        let before = awake_ms();
        std::thread::sleep(std::time::Duration::from_millis(15));
        assert!(before > 0);
        assert!(awake_ms() > before);
        let _ = events.take();
        assert!(events.registered());
        events.close().unwrap();
        assert!(!events.registered());
        events.close().unwrap();
    }
    #[test]
    fn failed_power_removal_retains_the_real_registration_until_retry() {
        let mut events = PowerEvents::register().unwrap();
        assert_eq!(events.close_with(|_| 5).unwrap_err().code, "power_error");
        assert!(events.registered() && events.state.is_some() && events.parameters.is_some());
        events.close().unwrap();
        assert!(!events.registered());
    }
}
