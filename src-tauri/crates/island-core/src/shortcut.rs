//! Layout-independent global shortcuts supported by the desktop adapter.
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    pub modifiers: u32,
    pub key: u32,
    pub label: String,
}
impl Shortcut {
    pub fn parse(value: &str) -> AppResult<Self> {
        let invalid = || {
            AppError::new("invalid_input", "Bitte Strg oder Alt, optional Umschalt und einen Buchstaben, eine Ziffer oder F1–F24 verwenden. F12 ist für Windows reserviert.")
        };
        if value.len() > 80 {
            return Err(invalid());
        }
        let parts: Vec<_> = value.split('+').map(str::trim).collect();
        if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
            return Err(invalid());
        }
        let mut modifiers = 0;
        for part in &parts[..parts.len() - 1] {
            let flag = match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "strg" => 2,
                "alt" => 1,
                "shift" | "umschalt" => 4,
                _ => return Err(invalid()),
            };
            if modifiers & flag != 0 {
                return Err(invalid());
            }
            modifiers |= flag;
        }
        if modifiers & 3 == 0 {
            return Err(invalid());
        }
        let last = parts.last().unwrap().to_ascii_uppercase();
        let key = if last.len() == 1 && last.as_bytes()[0].is_ascii_alphanumeric() {
            last.as_bytes()[0] as u32
        } else if let Some(number) = last.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
            if !(1..=24).contains(&number) || number == 12 || last != format!("F{number}") {
                return Err(invalid());
            }
            0x70 + number - 1
        } else {
            return Err(invalid());
        };
        let mut names = Vec::new();
        for (flag, name) in [(2, "Ctrl"), (1, "Alt"), (4, "Shift")] {
            if modifiers & flag != 0 {
                names.push(name);
            }
        }
        names.push(&last);
        Ok(Self {
            modifiers,
            key,
            label: names.join("+"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonicalizes_modifiers_and_function_keys() {
        let shortcut = Shortcut::parse(" alt + Strg + i ").unwrap();
        assert_eq!(shortcut.label, "Ctrl+Alt+I");
        assert_eq!((shortcut.modifiers, shortcut.key), (3, 0x49));
        assert_eq!(Shortcut::parse("Umschalt+Ctrl+F24").unwrap().key, 0x87);
        assert_eq!(Shortcut::parse("alt+7").unwrap().label, "Alt+7");
    }
    #[test]
    fn rejects_ambiguous_reserved_or_unmodified_input() {
        for invalid in [
            "I",
            "Shift+I",
            "Ctrl+Ctrl+I",
            "Ctrl++I",
            "Win+I",
            "Ctrl+F12",
            "Ctrl+F25",
            "Ctrl+F01",
            "Ctrl+ä",
            "Ctrl+Delete",
            "Alt+",
            "Ctrl+Alt",
        ] {
            assert!(Shortcut::parse(invalid).is_err(), "{invalid}");
        }
    }
}
