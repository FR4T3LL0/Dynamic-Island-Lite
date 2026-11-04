//! Shared serializable contracts and deterministic application logic.
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub mod audio;
pub mod display;
pub mod error;
pub mod media;
pub mod settings;
pub mod shortcut;
pub mod storage;
pub mod timer;
