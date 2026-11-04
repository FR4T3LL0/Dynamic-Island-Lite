//! Native Windows adapters. Real OS operations are implemented here.
#[cfg(windows)]
pub mod audio;
#[cfg(windows)]
pub mod autostart;
#[cfg(windows)]
pub mod display;
#[cfg(windows)]
pub mod display_events;
#[cfg(windows)]
pub mod hotkey;
#[cfg(windows)]
pub mod instance;
#[cfg(windows)]
pub mod media;
#[cfg(windows)]
pub mod power;
#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod shell;
#[cfg(windows)]
pub mod window;
