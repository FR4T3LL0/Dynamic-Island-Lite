use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub volume: f32,
    pub muted: bool,
    pub default_device: String,
    pub devices: Vec<AudioDevice>,
}

pub fn validate_volume(volume: f32) -> AppResult<f32> {
    if !volume.is_finite() || !(0.0..=100.0).contains(&volume) {
        return Err(AppError::new(
            "invalid_argument",
            "Die Lautstärke muss zwischen 0 und 100 liegen.",
        ));
    }
    Ok(volume / 100.0)
}
