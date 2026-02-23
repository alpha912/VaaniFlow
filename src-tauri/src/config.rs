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
    if !path.exists() {
        return Ok(Config::default());
    }
    let data = fs::read_to_string(&path)?;
    let config: Config = serde_json::from_str(&data)?;
    Ok(config)
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
