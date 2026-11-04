//! A single MTA owns media sessions. WinRT callbacks only wake its actor.
mod cover;
mod native;
mod service;
#[cfg(test)]
mod tests;
use island_core::{
    error::{AppError, AppResult},
    media::{MediaCommand, MediaSession},
};
pub use native::{MediaAdapter, SessionScope};
pub use service::{MediaCallback, MediaService, MediaSnapshot};
use std::{
    collections::BTreeMap,
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};
use windows_future::{AsyncStatus, IAsyncOperation};

const BACKGROUND_REFRESH_INTERVAL: Duration = Duration::from_millis(250);

pub type MediaChanged = Arc<dyn Fn() + Send + Sync>;
pub type CoverPool = BTreeMap<String, Arc<Vec<u8>>>;
#[derive(Default, Clone)]
pub struct BackendSnapshot {
    pub sessions: Vec<MediaSession>,
    pub current_id: Option<String>,
    pub covers: CoverPool,
}
pub trait MediaBackend {
    fn snapshot(&mut self, refresh_cover: bool) -> AppResult<BackendSnapshot>;
    fn has_pending_cover(&self) -> bool {
        false
    }
    fn execute(&mut self, id: &str, command: MediaCommand) -> AppResult<()>;
    fn subscriptions(&self) -> usize {
        0
    }
    fn close(&mut self) -> AppResult<()> {
        Ok(())
    }
}
pub fn media_error(error: windows::core::Error) -> AppError {
    AppError::new(
        "media_error",
        format!(
            "Windows konnte die Medienaktion nicht ausführen (Windows {:08X}).",
            error.code().0 as u32
        ),
    )
}
pub(crate) fn wait<T: windows::core::RuntimeType + 'static>(
    operation: &IAsyncOperation<T>,
    cancel: &AtomicBool,
    deadline: Instant,
) -> AppResult<T> {
    wait_status(
        || operation.Status(),
        || operation.Cancel(),
        cancel,
        deadline,
    )?;
    operation.GetResults().map_err(media_error)
}
fn wait_status(
    status: impl Fn() -> windows::core::Result<AsyncStatus>,
    cancel_operation: impl Fn() -> windows::core::Result<()>,
    cancel: &AtomicBool,
    deadline: Instant,
) -> AppResult<()> {
    use std::sync::atomic::Ordering;
    loop {
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            let _ = cancel_operation();
            return Err(AppError::new(
                "timeout",
                "Die Medienaktion wurde nicht rechtzeitig bestätigt.",
            ));
        }
        if status().map_err(media_error)? != AsyncStatus::Started {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
