use crate::error::{AppError, AppResult};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn unique_suffix() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn atomic_save<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|e| AppError::new("io_error", e.to_string()))?;
    bytes.push(b'\n');
    atomic_save_json_bytes(path, &bytes)
}

/// Restore a committed JSON document byte for byte, retaining the schema fence.
pub fn atomic_save_json_bytes(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let proposed: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| AppError::new("invalid_input", e.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Never replace a newer schema, including when a caller missed the load error.
    if let Ok(existing) = fs::read(path) {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&existing) {
            if json["schemaVersion"].as_u64().unwrap_or(0)
                > proposed["schemaVersion"].as_u64().unwrap_or(0)
            {
                return Err(AppError::new(
                    "unsupported",
                    "Eine neuere Konfigurationsversion wird nicht überschrieben.",
                ));
            }
        }
    }
    let temporary = path.with_extension(format!("tmp-{}", unique_suffix()));
    let result = (|| -> AppResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub struct Loaded<T> {
    pub value: T,
    pub warning: Option<String>,
}

pub fn load_versioned<T: DeserializeOwned + Default>(
    path: &Path,
    supported: u64,
) -> AppResult<Loaded<T>> {
    if !path.exists() {
        return Ok(Loaded {
            value: T::default(),
            warning: None,
        });
    }
    let bytes = fs::read(path)?;
    let parsed = serde_json::from_slice::<serde_json::Value>(&bytes);
    if let Ok(json) = &parsed {
        if json["schemaVersion"].as_u64().unwrap_or(0) > supported {
            return Err(AppError::new("unsupported", "Diese Daten wurden mit einer neueren App-Version gespeichert und bleiben unverändert."));
        }
    }
    let migrated = parsed.map(|mut json| {
        if supported == 1 && json["schemaVersion"].as_u64().unwrap_or(0) == 0 {
            if let Some(object) = json.as_object_mut() {
                object.insert("schemaVersion".into(), 1.into());
            }
        }
        json
    });
    match migrated.and_then(serde_json::from_value::<T>) {
        Ok(value) => Ok(Loaded {
            value,
            warning: None,
        }),
        Err(_) => {
            let backup = path.with_extension(format!("corrupt-{}.json", unique_suffix()));
            fs::rename(path, &backup)?;
            Ok(Loaded {
                value: T::default(),
                warning: Some(format!(
                    "Beschädigte Konfiguration gesichert: {}",
                    backup.display()
                )),
            })
        }
    }
}

pub fn write_new_file(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut file = File::create_new(path)?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    fn directory() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!("island-core-test-{}", unique_suffix()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }
    #[test]
    fn temporary_names_are_unique_across_concurrent_writes() {
        let workers: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (0..1_000).map(|_| unique_suffix()).collect::<Vec<_>>()))
            .collect();
        let names: std::collections::HashSet<_> = workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(names.len(), 8_000);
    }
    #[test]
    fn atomic_replace_preserves_latest_valid_values() {
        let directory = directory();
        let file = directory.join("settings.json");
        let mut settings = Settings::default();
        atomic_save(&file, &settings).unwrap();
        settings.width = 240;
        atomic_save(&file, &settings).unwrap();
        assert_eq!(
            load_versioned::<Settings>(&file, 1).unwrap().value.width,
            240
        );
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn newer_schema_is_never_overwritten() {
        let directory = directory();
        let file = directory.join("settings.json");
        let bytes = br#"{"schemaVersion":7,"privateOption":"keep"}"#;
        fs::write(&file, bytes).unwrap();
        assert!(load_versioned::<Settings>(&file, 1).is_err());
        assert!(atomic_save(&file, &Settings::default()).is_err());
        assert!(atomic_save_json_bytes(&file, br#"{"schemaVersion":1}"#).is_err());
        assert_eq!(fs::read(&file).unwrap(), bytes);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn json_rollback_preserves_original_bytes_and_rejects_invalid_input() {
        let directory = directory();
        let file = directory.join("settings.json");
        let original = b"{\"schemaVersion\": 1, \"width\":234}\r\n";
        atomic_save(&file, &Settings::default()).unwrap();
        atomic_save_json_bytes(&file, original).unwrap();
        assert_eq!(fs::read(&file).unwrap(), original);
        assert!(atomic_save_json_bytes(&file, b"broken").is_err());
        assert_eq!(fs::read(&file).unwrap(), original);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn corruption_is_preserved_and_old_schema_keeps_user_settings() {
        let directory = directory();
        let file = directory.join("settings.json");
        fs::write(&file, b"broken").unwrap();
        assert!(load_versioned::<Settings>(&file, 1)
            .unwrap()
            .warning
            .is_some());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::write(
            &file,
            br##"{"schemaVersion":0,"width":234,"accent":"#F0AABB"}"##,
        )
        .unwrap();
        let settings = load_versioned::<Settings>(&file, 1).unwrap().value;
        assert_eq!(settings.schema_version, 1);
        assert_eq!(settings.width, 234);
        assert_eq!(settings.accent, "#F0AABB");
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn failed_replace_keeps_the_destination_and_removes_temp() {
        let directory = directory();
        let destination = directory.join("settings.json");
        fs::create_dir(&destination).unwrap();
        assert!(atomic_save(&destination, &Settings::default()).is_err());
        assert!(destination.is_dir());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
