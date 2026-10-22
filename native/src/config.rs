//! Non-secret user preferences. The Sarvam credential is stored separately in
//! the operating-system keyring and is never serialized to this file.

use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

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
    let path = config_path();
    let parent = path.parent().ok_or("Invalid config path")?;
    fs::create_dir_all(parent)?;
    let data = serde_json::to_vec_pretty(config)?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, data)?;
    // Rename is atomic on the same volume and avoids truncating a valid config
    // if the process exits during a write.
    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(temp_path, path)?;
    Ok(())
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
}
