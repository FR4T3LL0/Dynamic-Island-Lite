use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Capsule,
    Notch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CoverTransition {
    #[default]
    Fade,
    Slide,
    Zoom,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub schema_version: u32,
    pub appearance: Appearance,
    pub width: u32,
    pub top_gap: u32,
    pub accent: String,
    pub reduced_motion: bool,
    pub monitor: Option<String>,
    pub shortcut: String,
    pub clock_follows_island: bool,
    pub peek: bool,
    pub edge_delay_ms: u32,
    // Preserve the old default for existing documents that omit this field.
    #[serde(default)]
    pub smart: bool,
    pub fixed_appbar: bool,
    pub visualizer: bool,
    pub media_color_accent: bool,
    pub cover_transition: CoverTransition,
    pub autostart: bool,
    pub timer_sound: bool,
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn smart_default_does_not_change_existing_visibility_choices() {
        assert!(Settings::default().smart);
        for document in [
            r#"{"smart":false}"#,
            r#"{"peek":true}"#,
            r#"{"fixedAppbar":true}"#,
        ] {
            let settings: Settings = serde_json::from_str(document).unwrap();
            assert!(!settings.smart);
            assert!(settings.validate().is_ok());
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            appearance: Appearance::Capsule,
            width: 180,
            top_gap: 12,
            accent: "#2997FF".into(),
            reduced_motion: false,
            monitor: None,
            shortcut: "Ctrl+Alt+Shift+I".into(),
            clock_follows_island: false,
            peek: false,
            edge_delay_ms: 300,
            smart: true,
            fixed_appbar: false,
            visualizer: true,
            media_color_accent: true,
            cover_transition: CoverTransition::default(),
            autostart: true,
            timer_sound: true,
        }
    }
}

impl Settings {
    pub fn compact_width(&self) -> f64 {
        f64::from(self.width)
            + if self.appearance == Appearance::Notch {
                20.0
            } else {
                0.0
            }
    }

    pub fn island_gap(&self) -> f64 {
        if self.appearance == Appearance::Notch {
            0.0
        } else {
            f64::from(self.top_gap)
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.schema_version != 1 {
            return Err(AppError::new(
                "unsupported",
                "Die Konfigurationsversion wird nicht unterstützt.",
            ));
        }
        if !(140..=320).contains(&self.width) || self.top_gap > 48 {
            return Err(AppError::new(
                "invalid_input",
                "Breite oder Randabstand liegt außerhalb des erlaubten Bereichs.",
            ));
        }
        if self.accent.len() != 7
            || !self.accent.starts_with('#')
            || !self.accent[1..].bytes().all(|ch| ch.is_ascii_hexdigit())
        {
            return Err(AppError::new(
                "invalid_input",
                "Die Akzentfarbe muss eine sechsstellige Hexfarbe sein.",
            ));
        }
        if [self.peek, self.smart, self.fixed_appbar]
            .into_iter()
            .filter(|enabled| *enabled)
            .count()
            > 1
        {
            return Err(AppError::new(
                "invalid_input",
                "Bitte genau einen Sichtbarkeitsmodus auswählen.",
            ));
        }
        if self.edge_delay_ms > 1500 {
            return Err(AppError::new(
                "invalid_input",
                "Die Randverzögerung darf höchstens 1500 ms betragen.",
            ));
        }
        crate::shortcut::Shortcut::parse(&self.shortcut)?;
        Ok(())
    }
}
