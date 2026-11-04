//! Current-user Run registration. No task, service or elevation is required.
use island_core::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::*,
    },
};
pub const VALUE_NAME: &str = "app.dynamicisland.lite";
pub const TEST_PREFIX: &str = "app.dynamicisland.lite.test.";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawValue {
    pub kind: u32,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub registered: bool,
    pub error: Option<AppError>,
}
pub struct Autostart {
    name: String,
    command: String,
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn error(message: impl Into<String>) -> AppError {
    AppError::new("io_error", message)
}
fn key(write: bool) -> AppResult<Option<Key>> {
    let mut result = HKEY::default();
    let code = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            if write {
                KEY_QUERY_VALUE | KEY_SET_VALUE
            } else {
                KEY_QUERY_VALUE
            },
            &mut result,
        )
    };
    if code == ERROR_FILE_NOT_FOUND && !write {
        return Ok(None);
    }
    if code == ERROR_FILE_NOT_FOUND {
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                None,
                &mut result,
                None,
            )
        }
        .ok()
        .map_err(|e| error(e.to_string()))?;
    } else {
        code.ok().map_err(|e| error(e.to_string()))?;
    }
    Ok(Some(Key(result)))
}
impl Autostart {
    pub fn new(exe: &Path, test_name: Option<&str>) -> AppResult<Self> {
        let name = test_name.unwrap_or(VALUE_NAME);
        if test_name.is_some()
            && (!name.starts_with(TEST_PREFIX)
                || name.len() > 160
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c)))
        {
            return Err(error("Ungültiger Autostart-Testname."));
        }
        let path = exe
            .to_str()
            .ok_or_else(|| error("Der Programmpfad ist nicht als Unicode darstellbar."))?;
        if !exe.is_absolute() || path.contains(['"', '\0', '\r', '\n']) {
            return Err(error("Der Programmpfad ist für Autostart ungültig."));
        }
        Ok(Self {
            name: name.into(),
            command: format!("\"{path}\" --autostart"),
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    fn expected(&self) -> RawValue {
        RawValue {
            kind: REG_SZ.0,
            bytes: wide(&self.command)
                .iter()
                .flat_map(|c| c.to_le_bytes())
                .collect(),
        }
    }
    pub fn read(&self) -> AppResult<Option<RawValue>> {
        let Some(key) = key(false)? else {
            return Ok(None);
        };
        let name = wide(&self.name);
        let mut length = 0;
        let mut kind = REG_VALUE_TYPE(0);
        let code = unsafe {
            RegQueryValueExW(
                key.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut length),
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        code.ok().map_err(|e| error(e.to_string()))?;
        if length > 16_384 {
            return Err(error(
                "Der vorhandene Autostarteintrag ist ungewöhnlich groß und bleibt unverändert.",
            ));
        }
        let mut bytes = vec![0u8; length as usize];
        unsafe {
            RegQueryValueExW(
                key.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                Some(bytes.as_mut_ptr()),
                Some(&mut length),
            )
        }
        .ok()
        .map_err(|e| error(e.to_string()))?;
        bytes.truncate(length as usize);
        Ok(Some(RawValue {
            kind: kind.0,
            bytes,
        }))
    }
    fn write(&self, value: Option<&RawValue>) -> AppResult<()> {
        let Some(key) = key(true)? else {
            return Err(error("Der Autostartschlüssel fehlt."));
        };
        let name = wide(&self.name);
        let code = if let Some(value) = value {
            unsafe {
                RegSetValueExW(
                    key.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    REG_VALUE_TYPE(value.kind),
                    Some(&value.bytes),
                )
            }
        } else {
            unsafe { RegDeleteValueW(key.0, PCWSTR(name.as_ptr())) }
        };
        if code != ERROR_SUCCESS && !(value.is_none() && code == ERROR_FILE_NOT_FOUND) {
            code.ok().map_err(|e| error(e.to_string()))?;
        }
        if self.read()?.as_ref() != value {
            return Err(error("Windows hat den Autostarteintrag nicht bestätigt."));
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Status {
        match self.read() {
            Ok(value) => Status {
                registered: value.as_ref() == Some(&self.expected()),
                error: if value.is_some() && value.as_ref() != Some(&self.expected()) {
                    Some(error("Der vorhandene Autostarteintrag zeigt auf einen anderen Programmstand. Er wird erst beim Speichern der Einstellung angepasst."))
                } else {
                    None
                },
            },
            Err(error) => Status {
                registered: false,
                error: Some(error),
            },
        }
    }
    pub fn change<T>(&self, enabled: bool, persist: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
        if enabled && self.command.encode_utf16().count() > 260 {
            return Err(error(
                "Der Programmpfad ist für den Windows-Autostarteintrag zu lang.",
            ));
        }
        let previous = self.read()?;
        let next = enabled.then(|| self.expected());
        if previous != next {
            if let Err(failure) = self.write(next.as_ref()) {
                return match self.write(previous.as_ref()) {Ok(())=>Err(failure),Err(rollback)=>Err(error(format!("{failure} Wiederherstellung des Autostarteintrags fehlgeschlagen: {rollback}")))};
            }
        }
        match persist() {
            Ok(value) => Ok(value),
            Err(failure) => {
                if previous != next {
                    if self.read()? != next {
                        return Err(error(format!("{failure} Der Autostarteintrag wurde gleichzeitig außerhalb der App verändert; bitte die Einstellung erneut prüfen.")));
                    }
                    self.write(previous.as_ref())?;
                }
                Err(failure)
            }
        }
    }
    /// Query exact ownership without changing the current user's Run value.
    pub fn is_owned_registration(&self) -> AppResult<bool> {
        Ok(self.read()?.as_ref() == Some(&self.expected()))
    }
    /// Recreate a previously owned entry only after installation succeeded.
    /// A foreign value is never overwritten by installer reconciliation.
    pub fn restore_install_registration(&self) -> AppResult<bool> {
        match self.read()? {
            Some(value) if value == self.expected() => return Ok(false),
            Some(_) => return Err(error("Ein fremder Autostarteintrag bleibt unverändert; der Installationsabschluss wurde angehalten.")),
            None => {}
        }
        if self.command.encode_utf16().count() > 260 {
            return Err(error(
                "Der Programmpfad ist für den Windows-Autostarteintrag zu lang.",
            ));
        }
        self.write(Some(&self.expected()))?;
        if !self.is_owned_registration()? {
            return Err(error(
                "Der eigene Autostarteintrag konnte nach der Installation nicht bestätigt werden.",
            ));
        }
        Ok(true)
    }
    /// Uninstall removes only the registration matching this exact executable.
    pub fn remove_owned_registration(&self) -> AppResult<bool> {
        if self.read()?.as_ref() != Some(&self.expected()) {
            return Ok(false);
        }
        self.write(None)?;
        if self.read()?.is_some() {
            return Err(error("Der eigene Autostarteintrag wurde gleichzeitig verändert und konnte nicht bestätigt entfernt werden."));
        }
        Ok(true)
    }
    /// Cleanup for an explicitly isolated native test entry, never a product entry.
    pub fn remove_test_entry(&self) -> AppResult<()> {
        if !self.name.starts_with(TEST_PREFIX) {
            return Err(error(
                "Nur eigene Testeinträge dürfen so aufgeräumt werden.",
            ));
        }
        self.write(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_user_run_roundtrip_readback_and_storage_failure_rollback() {
        let name = format!("{TEST_PREFIX}{}", crate::process::random_id().unwrap());
        let autostart = Autostart::new(&std::env::current_exe().unwrap(), Some(&name)).unwrap();
        struct Cleanup<'a>(&'a Autostart);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                self.0.remove_test_entry().unwrap();
            }
        }
        let _cleanup = Cleanup(&autostart);
        assert_eq!(autostart.read().unwrap(), None);
        let previous = RawValue {
            kind: REG_BINARY.0,
            bytes: vec![9, 8, 7, 0, 255],
        };
        autostart.write(Some(&previous)).unwrap();
        assert!(autostart
            .change(true, || Err::<(), _>(error("Speicherfehler")))
            .is_err());
        assert_eq!(autostart.read().unwrap(), Some(previous.clone()));
        assert!(!autostart.remove_owned_registration().unwrap());
        assert!(!autostart.is_owned_registration().unwrap());
        assert!(autostart.restore_install_registration().is_err());
        assert_eq!(autostart.read().unwrap(), Some(previous));
        autostart.remove_test_entry().unwrap();
        assert!(autostart.restore_install_registration().unwrap());
        assert!(!autostart.restore_install_registration().unwrap());
        assert!(autostart.is_owned_registration().unwrap());
        assert!(autostart.snapshot().registered);
        assert!(autostart.remove_owned_registration().unwrap());
        assert!(!autostart.remove_owned_registration().unwrap());
        autostart.change(true, || Ok(())).unwrap();
        assert!(autostart
            .change(false, || Err::<(), _>(error("Speicherfehler")))
            .is_err());
        assert!(autostart.snapshot().registered);
        autostart.change(false, || Ok(())).unwrap();
        assert_eq!(autostart.read().unwrap(), None);
        assert!(autostart
            .change(true, || Err::<(), _>(error("Speicherfehler")))
            .is_err());
        assert_eq!(autostart.read().unwrap(), None);
    }
}
