use island_core::error::{AppError, AppResult};
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::PathBuf,
};
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{FILETIME, HANDLE, WAIT_OBJECT_0},
        System::{
            Com::CoCreateGuid,
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{
                GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
                PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            },
        },
    },
};
fn error(context: &str, e: windows::core::Error) -> AppError {
    AppError::new(
        "process_io",
        format!("{context} (Windows {:08X}).", e.code().0 as u32),
    )
}
fn owned(handle: HANDLE) -> OwnedHandle {
    unsafe { OwnedHandle::from_raw_handle(handle.0) }
}
fn raw(handle: &OwnedHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}
pub fn random_id() -> AppResult<String> {
    unsafe { CoCreateGuid() }
        .map(|id| format!("{:032x}", id.to_u128()))
        .map_err(|e| error("Die Laufkennung konnte nicht erstellt werden", e))
}
fn creation_time(handle: HANDLE) -> AppResult<u64> {
    let mut start = FILETIME::default();
    let mut end = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { GetProcessTimes(handle, &mut start, &mut end, &mut kernel, &mut user) }
        .map_err(|e| error("Der Elternprozess konnte nicht geprüft werden", e))?;
    Ok(((start.dwHighDateTime as u64) << 32) | start.dwLowDateTime as u64)
}
pub struct ProcessWatch {
    handle: OwnedHandle,
    pub started_at: u64,
}
impl ProcessWatch {
    pub fn executable_path(&self) -> AppResult<PathBuf> {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                raw(&self.handle),
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
        }
        .map_err(|e| error("Der Prozesspfad konnte nicht geprüft werden", e))?;
        Ok(PathBuf::from(OsString::from_wide(
            &buffer[..length as usize],
        )))
    }
    pub fn open(pid: u32, expected_start: Option<u64>) -> AppResult<Self> {
        let handle = owned(
            unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    false,
                    pid,
                )
            }
            .map_err(|e| error("Der Elternprozess ist nicht verfügbar", e))?,
        );
        let started_at = creation_time(raw(&handle))?;
        let mut session = 0;
        unsafe { ProcessIdToSessionId(pid, &mut session) }
            .map_err(|e| error("Die Elternprozess-Sitzung konnte nicht geprüft werden", e))?;
        if session != crate::window::current_session()? {
            return Err(AppError::new(
                "access_denied",
                "Der Prozess gehört zu einer anderen Windows-Sitzung.",
            ));
        }
        if expected_start.is_some_and(|expected| expected != started_at) {
            return Err(AppError::new(
                "stale_window",
                "Die Identität des Elternprozesses hat sich geändert.",
            ));
        }
        Ok(Self { handle, started_at })
    }
    pub fn has_exited(&self) -> bool {
        (unsafe { WaitForSingleObject(raw(&self.handle), 0) }) == WAIT_OBJECT_0
    }
}
