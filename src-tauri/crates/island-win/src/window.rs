use island_core::error::{AppError, AppResult};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{HANDLE, HWND},
        System::RemoteDesktop::ProcessIdToSessionId,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
};

fn os_error(context: &str, error: windows::core::Error) -> AppError {
    let code = if error.code().0 as u32 == 0x80070005 {
        "access_denied"
    } else {
        "window_error"
    };
    AppError::new(
        code,
        format!("{context} (Windows {:08X}).", error.code().0 as u32),
    )
}
pub(crate) struct DpiContext(DPI_AWARENESS_CONTEXT);
impl DpiContext {
    pub(crate) fn physical() -> AppResult<Self> {
        let old =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        // -1 is the valid UNAWARE context; only NULL means this API failed.
        if old.0.is_null() {
            return Err(os_error(
                "Physische Fensterkoordinaten sind nicht verfügbar",
                windows::core::Error::from_thread(),
            ));
        }
        Ok(Self(old))
    }
}
impl Drop for DpiContext {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}

pub fn current_session() -> AppResult<u32> {
    session_for(std::process::id())
}
fn session_for(pid: u32) -> AppResult<u32> {
    let mut session = 0;
    unsafe { ProcessIdToSessionId(pid, &mut session) }
        .map_err(|e| os_error("Die Windows-Sitzung konnte nicht geprüft werden", e))?;
    Ok(session)
}
pub struct FocusTarget {
    hwnd: usize,
    property: Vec<u16>,
}
impl FocusTarget {
    pub fn remember(overlay: HWND) -> AppResult<Option<Self>> {
        let target = unsafe { GetForegroundWindow() };
        if target.is_invalid() || target == overlay || !unsafe { IsWindowVisible(target) }.as_bool()
        {
            return Ok(None);
        }
        let property: Vec<u16> =
            format!("DynamicIslandLite.Focus.{}", crate::process::random_id()?)
                .encode_utf16()
                .chain(Some(0))
                .collect();
        let token = Self {
            hwnd: target.0 as usize,
            property,
        };
        unsafe {
            SetPropW(
                target,
                PCWSTR(token.property.as_ptr()),
                Some(HANDLE(1 as _)),
            )
        }
        .map_err(|error| os_error("Das vorige Fenster konnte nicht markiert werden", error))?;
        Ok(Some(token))
    }

    pub fn return_focus(self, overlay: HWND) -> AppResult<()> {
        let current = unsafe { GetForegroundWindow() };
        let target = HWND(self.hwnd as _);
        if (current.is_invalid() || current == overlay)
            && self.owns_marker()
            && unsafe { IsWindowVisible(target) }.as_bool()
            && !unsafe { IsIconic(target) }.as_bool()
            && !unsafe { SetForegroundWindow(target) }.as_bool()
        {
            return Err(AppError::new(
                "window_error",
                "Der Fokus konnte nicht an das vorige Fenster zurückgegeben werden.",
            ));
        }
        Ok(())
    }

    fn owns_marker(&self) -> bool {
        (unsafe { GetPropW(HWND(self.hwnd as _), PCWSTR(self.property.as_ptr())) })
            == HANDLE(1 as _)
    }
}
impl Drop for FocusTarget {
    fn drop(&mut self) {
        if self.owns_marker() {
            let _ = unsafe { RemovePropW(HWND(self.hwnd as _), PCWSTR(self.property.as_ptr())) };
        }
    }
}
