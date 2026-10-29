//! Non-secret user preferences. The Sarvam credential is stored separately in
//! the operating-system keyring and is never serialized to this file.

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const KEYRING_SERVICE: &str = "VaaniFlow";
const KEYRING_USER: &str = "sarvam-api-key";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// `auto` or a BCP-47 language code.
    pub source_language: String,
    /// `same` or a BCP-47 target language code.
    pub target_language: String,
    /// e.g. `Ctrl+Shift+Space`.
    pub hotkey: String,
    /// Device name, or `default`.
    pub microphone: String,
    /// Sarvam Saaras REST model; v4 can be selected but should be validated with
    /// the user's account/region before being made the default.
    pub stt_model: String,
    /// Output mode supported by the selected Saaras model.
    pub stt_mode: String,
    /// Mayura translation style.
    pub translation_mode: String,
    /// Comma-separated terminology hints. Saaras v4 supports up to 50 terms.
    pub keyterms: String,
    /// Require a review/edit before text is inserted into another app.
    pub review_before_paste: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source_language: "auto".into(),
            target_language: "same".into(),
            hotkey: "Ctrl+Shift+Space".into(),
            microphone: "default".into(),
            stt_model: "saaras:v3".into(),
            stt_mode: "transcribe".into(),
            translation_mode: "formal".into(),
            keyterms: String::new(),
            review_before_paste: true,
        }
    }
}

impl Config {
    /// Normalize untrusted or outdated values before passing them to the API.
    pub fn normalize(&mut self) {
        self.hotkey = normalize_hotkey(&self.hotkey);
        if !matches!(self.stt_model.as_str(), "saaras:v3" | "saaras:v4") {
            self.stt_model = "saaras:v3".into();
        }
        if !matches!(
            self.stt_mode.as_str(),
            "transcribe" | "translate" | "verbatim" | "translit" | "codemix"
        ) {
            self.stt_mode = "transcribe".into();
        }
        if !matches!(
            self.translation_mode.as_str(),
            "formal" | "classic-colloquial" | "modern-colloquial" | "code-mixed"
        ) {
            self.translation_mode = "formal".into();
        }
        self.keyterms = self.keyterms.trim().to_owned();
    }
}

#[derive(Debug)]
pub struct LoadedConfig {
    pub config: Config,
    pub api_key: String,
    /// Set when secure credential storage is unavailable. The key remains in
    /// memory for the current session, but will need to be entered again.
    pub credential_warning: Option<String>,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vaaniflow")
        .join("config.json")
}

/// The v0.2 Tauri app persisted settings under this directory. Read it once so
/// its settings and plaintext API key can be migrated into the native app.
fn legacy_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("vaani")
        .join("config.json")
}

fn keyring_entry() -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
}

fn deserialize_config(data: &str) -> Result<(Config, Option<String>), serde_json::Error> {
    let mut value: serde_json::Value = serde_json::from_str(data)?;
    let legacy_key = value
        .as_object_mut()
        .and_then(|object| object.remove("api_key"))
        .and_then(|value| value.as_str().map(str::to_owned));
    let config = serde_json::from_value(value)?;
    Ok((config, legacy_key))
}

pub fn load() -> Result<LoadedConfig, Box<dyn std::error::Error>> {
    let current_path = config_path();
    let old_path = legacy_config_path();
    let (path, is_legacy) = if current_path.exists() {
        (current_path, false)
    } else if old_path.exists() {
        (old_path.clone(), true)
    } else {
        (current_path, false)
    };
    let mut legacy_key = None;
    let mut config = if path.exists() {
        let data = fs::read_to_string(&path)?;
        // v0.2 persisted api_key in plaintext. Migrate it to the OS keyring
        // before rewriting the file, and leave the original untouched on error.
        let (config, key) = deserialize_config(&data)?;
        legacy_key = key;
        config
    } else {
        Config::default()
    };
    config.normalize();

    let has_legacy_key = legacy_key.as_ref().is_some_and(|key| !key.is_empty());
    let (api_key, mut credential_warning, migrated) = match keyring_entry() {
        Ok(entry) => match entry.get_password() {
            Ok(key) => (key, None, true),
            Err(keyring::Error::NoEntry) => match legacy_key {
                Some(key) if !key.is_empty() => match entry.set_password(&key) {
                    Ok(()) => (key, None, true),
                    Err(error) => (
                        key,
                        Some(format!("Could not migrate the API key to the OS keyring: {error}. It is available only for this session.")),
                        false,
                    ),
                },
                _ => (String::new(), None, true),
            },
            Err(error) => (
                legacy_key.unwrap_or_default(),
                Some(format!("Could not access the OS keyring: {error}. The API key is available only for this session.")),
                false,
            ),
        },
        Err(error) => (
            legacy_key.unwrap_or_default(),
            Some(format!("Could not initialize the OS keyring: {error}. The API key is available only for this session.")),
            false,
        ),
    };

    // Persist the canonical config only after successful keyring migration.
    // When loading the old v0.2 location, remove that copy only after the new
    // preferences have been written successfully.
    if path.exists() && migrated {
        save(&config)?;
        if is_legacy {
            if let Err(error) = fs::remove_file(&old_path) {
                credential_warning = Some(format!(
                    "The API key was migrated, but the old settings file could not be removed: {error}. Delete {} manually.",
                    old_path.display()
                ));
            }
        }
    } else if is_legacy && has_legacy_key {
        let warning = format!(
            "The old settings file still contains the API key because secure migration failed. Delete {} after copying the key somewhere safe.",
            old_path.display()
        );
        credential_warning = Some(match credential_warning {
            Some(existing) => format!("{existing} {warning}"),
            None => warning,
        });
    }

    Ok(LoadedConfig {
        config,
        api_key,
        credential_warning,
    })
}

pub fn save_api_key(key: &str) -> Result<(), keyring::Error> {
    let entry = keyring_entry()?;
    if key.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error),
        }
    } else {
        entry.set_password(key)
    }
}

pub fn save(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    save_to_path(&config_path(), config)
}

fn save_to_path(path: &Path, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path.parent().ok_or("Invalid config path")?;
    fs::create_dir_all(parent)?;
    let data = serde_json::to_vec_pretty(config)?;
    static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);
    let file_name = path.file_name().ok_or("Invalid config file name")?;
    let temp_name = format!(
        ".{}.{}.{}.tmp",
        file_name.to_string_lossy(),
        std::process::id(),
        SAVE_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let temp_path = parent.join(temp_name);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temp_path, path)?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| std::io::Error::other(error.to_string()))
}

fn normalize_hotkey(hotkey: &str) -> String {
    hotkey
        .split('+')
        .map(|part| {
            let mut chars = part.trim().chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_review_first_and_do_not_contain_a_credential() {
        let config = Config::default();
        assert!(config.review_before_paste);
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("api_key"));
        assert!(!json.contains("password"));
    }

    #[test]
    fn old_settings_deserialize_with_defaults() {
        let old =
            r#"{"source_language":"hi-IN","target_language":"same","hotkey":"ctrl+shift+space"}"#;
        let config: Config = serde_json::from_str(old).unwrap();
        assert_eq!(config.source_language, "hi-IN");
        assert!(config.review_before_paste);
        assert_eq!(normalize_hotkey(&config.hotkey), "Ctrl+Shift+Space");
    }

    #[test]
    fn legacy_plaintext_key_is_separated_from_preferences() {
        let old =
            r#"{"api_key":"example-secret","source_language":"ta-IN","target_language":"same"}"#;
        let (config, key) = deserialize_config(old).unwrap();
        assert_eq!(key.as_deref(), Some("example-secret"));
        assert_eq!(config.source_language, "ta-IN");
        let preferences = serde_json::to_string(&config).unwrap();
        assert!(!preferences.contains("example-secret"));
        assert!(!preferences.contains("api_key"));
    }

    #[test]
    fn invalid_api_preferences_are_safely_normalized() {
        let mut config: Config = serde_json::from_str(
            r#"{"stt_model":"custom","stt_mode":"raw","translation_mode":"casual"}"#,
        )
        .unwrap();
        config.normalize();
        assert_eq!(config.stt_model, "saaras:v3");
        assert_eq!(config.stt_mode, "transcribe");
        assert_eq!(config.translation_mode, "formal");
    }

    fn test_directory() -> PathBuf {
        static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "vaaniflow-config-test-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn settings_replace_the_previous_file() {
        let directory = test_directory();
        let path = directory.join("config.json");
        fs::write(&path, br#"{"source_language":"ta-IN"}"#).unwrap();
        let config = Config {
            source_language: "hi-IN".into(),
            ..Config::default()
        };

        save_to_path(&path, &config).unwrap();

        let saved: Config = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.source_language, "hi-IN");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_replacement_keeps_the_existing_target_intact() {
        let directory = test_directory();
        let path = directory.join("config.json");
        fs::create_dir(&path).unwrap();
        let sentinel = path.join("keep.txt");
        fs::write(&sentinel, b"existing data").unwrap();

        assert!(save_to_path(&path, &Config::default()).is_err());
        assert_eq!(fs::read(sentinel).unwrap(), b"existing data");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
