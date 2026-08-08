//! Config file persistence for the athan CLI.
//!
//! Port of `config.js` from the Node athan-cli.
//!
//! Config location:
//!   - Linux/macOS: $XDG_CONFIG_HOME/athan/config.json (fallback ~/.config/athan/config.json)
//!   - Windows:     %APPDATA%/athan/config.json

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// The available athan sounds (ported from the Node config.js / athan-web settings.js).
pub struct SoundInfo {
    pub key: &'static str,
    pub label: &'static str,
    pub size: &'static str,
}

pub const ATHAN_SOUNDS: &[SoundInfo] = &[
    SoundInfo { key: "adhan-alaqsa", label: "Al-Aqsa", size: "362 KB" },
    SoundInfo { key: "adhan-makkah", label: "Makkah", size: "3.4 MB" },
    SoundInfo { key: "adhan-madinah", label: "Madinah", size: "3.4 MB" },
    SoundInfo { key: "adhan-mishary", label: "Mishary Rashid", size: "1.9 MB" },
];

pub const FAJR_SOUNDS: &[SoundInfo] = &[SoundInfo {
    key: "adhan-fajr",
    label: "Al-Aqsa Fajr",
    size: "484 KB",
}];

pub fn all_sounds() -> impl Iterator<Item = &'static SoundInfo> {
    ATHAN_SOUNDS.iter().chain(FAJR_SOUNDS.iter())
}

pub fn is_valid_sound(key: &str) -> bool {
    all_sounds().any(|s| s.key == key)
}

/// Config schema, identical to the Node CLI (camelCase JSON keys).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_madhab")]
    pub madhab: String,
    #[serde(default = "default_athan_sound")]
    pub athan_sound: String,
    #[serde(default = "default_fajr_sound")]
    pub fajr_sound: String,
    #[serde(default = "default_true")]
    pub athan_enabled: bool,
}

fn default_method() -> String {
    "NorthAmerica".to_string()
}
fn default_madhab() -> String {
    "Shafi".to_string()
}
fn default_athan_sound() -> String {
    "adhan-alaqsa".to_string()
}
fn default_fajr_sound() -> String {
    "adhan-fajr".to_string()
}
fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config {
            latitude: None,
            longitude: None,
            method: default_method(),
            madhab: default_madhab(),
            athan_sound: default_athan_sound(),
            fajr_sound: default_fajr_sound(),
            athan_enabled: true,
        }
    }
}

/// The config keys in the same order the Node CLI prints them.
pub const VALID_KEYS: &[&str] = &[
    "latitude",
    "longitude",
    "method",
    "madhab",
    "athanSound",
    "fajrSound",
    "athanEnabled",
];

/// Resolve the config directory for the current platform.
pub fn config_dir() -> PathBuf {
    // dirs::config_dir() matches the Node resolution: $XDG_CONFIG_HOME /
    // ~/.config on Linux/macOS, %APPDATA% (Roaming) on Windows.
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("athan")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn state_path() -> PathBuf {
    config_dir().join("state.json")
}

/// Load config, merged with defaults (missing keys fall back to defaults).
pub fn load_config() -> Config {
    match fs::read_to_string(config_path()) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

/// Save config (full object).
pub fn save_config(config: &Config) -> Result<(), AppError> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(config)? + "\n";
    fs::write(config_path(), json)?;
    Ok(())
}

/// Set a single config key with the same coercion/validation as the Node CLI.
/// Returns the updated config.
pub fn set_config_value(key: &str, value: &str) -> Result<Config, AppError> {
    if !VALID_KEYS.contains(&key) {
        return Err(AppError::msg(format!(
            "Unknown config key \"{key}\". Valid keys: {}",
            VALID_KEYS.join(", ")
        )));
    }

    let mut config = load_config();
    match key {
        "latitude" | "longitude" => {
            let n: f64 = value
                .parse()
                .ok()
                .filter(|n: &f64| n.is_finite())
                .ok_or_else(|| AppError::msg(format!("Invalid {key}: \"{value}\"")))?;
            let limit = if key == "latitude" { 90.0 } else { 180.0 };
            if n.abs() > limit {
                return Err(AppError::msg(format!("Invalid {key}: \"{value}\"")));
            }
            if key == "latitude" {
                config.latitude = Some(n);
            } else {
                config.longitude = Some(n);
            }
        }
        "athanEnabled" => {
            config.athan_enabled = match value {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(AppError::msg("athanEnabled must be \"true\" or \"false\""));
                }
            };
        }
        "method" => config.method = value.to_string(),
        "madhab" => config.madhab = value.to_string(),
        "athanSound" => config.athan_sound = value.to_string(),
        "fajrSound" => config.fajr_sound = value.to_string(),
        _ => unreachable!(),
    }

    save_config(&config)?;
    Ok(config)
}

// ---------------------------------------------------------------------------
// Scheduler state (lastPlayed tracking)
// ---------------------------------------------------------------------------

/// Shape: `{ "lastPlayed": { "2026-08-06": ["fajr", "dhuhr"] } }`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    #[serde(default)]
    pub last_played: BTreeMap<String, Vec<String>>,
}

pub fn load_state() -> State {
    match fs::read_to_string(state_path()) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => State::default(),
    }
}

pub fn save_state(state: &State) -> Result<(), AppError> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(state)? + "\n";
    fs::write(state_path(), json)?;
    Ok(())
}
