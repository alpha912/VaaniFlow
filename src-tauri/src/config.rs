// src-tauri/src/config.rs
// Persists user settings to the OS app data directory

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    /// "auto" or a BCP-47 code like "hi-IN"
    pub source_language: String,
    /// "same" = no translation, or target BCP-47 code
    pub target_language: String,
    /// e.g. "ctrl+shift+space" or "cmd+shift+v"
    pub hotkey: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            source_language: "auto".to_string(),
            target_language: "same".to_string(),
            hotkey: "ctrl+shift+space".to_string(),
        }
    }
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("vaani");
    path.push("config.json");
    path
}

pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
    let path = config_path();
    let mut config = if path.exists() {
        let data = fs::read_to_string(&path)?;
        serde_json::from_str(&data)?
    } else {
        Config::default()
    };

    // Dev-only: if no API key saved, try reading from .env.development
    if config.api_key.is_empty() {
        if let Some(key) = dev_api_key() {
            config.api_key = key;
        }
    }

    Ok(config)
}

/// In debug builds, read VITE_SARVAM_API_KEY from the project's .env.development.
/// CARGO_MANIFEST_DIR points to src-tauri; its parent is the project root.
#[cfg(debug_assertions)]
fn dev_api_key() -> Option<String> {
    let env_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join(".env.development");
    let content = fs::read_to_string(env_file).ok()?;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("VITE_SARVAM_API_KEY=") {
            return Some(val.trim().to_string());
        }
    }
    None
}

#[cfg(not(debug_assertions))]
fn dev_api_key() -> Option<String> {
    None
}

pub fn save(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(config)?;
    fs::write(&path, data)?;
    Ok(())
}
