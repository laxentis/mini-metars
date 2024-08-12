use crate::profiles::{get_latest_profile_path, load_profile_from_path, Profile};
use crate::utils;
use crate::utils::deserialize_from_file;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub load_most_recent_profile_on_open: bool,
    pub most_recent_profile: Option<PathBuf>,
}

impl Settings {
    pub const fn new() -> Self {
        Self {
            load_most_recent_profile_on_open: true,
            most_recent_profile: None,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_local_dir().map(|p| p.join("Mini METARs").join("settings.json"))
}

fn read_settings_or_default() -> Settings {
    settings_path().map_or_else(Settings::default, |p| {
        deserialize_from_file(&p).unwrap_or_else(|_| Settings::default())
    })
}

fn write_settings_to_file(settings: &Settings) -> Result<(), anyhow::Error> {
    settings_path().map_or_else(
        || Err(anyhow!("Could not construct path to settings.json")),
        |p| utils::serialize_to_file(&p, settings),
    )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitialSettingsLoadResponse {
    pub settings: Settings,
    pub profile: Option<Profile>,
}

#[tauri::command(async)]
pub fn load_settings_initial(app: AppHandle) -> Result<InitialSettingsLoadResponse, String> {
    let settings = read_settings_or_default();
    let mut profile = None;
    if settings.load_most_recent_profile_on_open {
        if let Some(path) = settings.most_recent_profile.as_ref() {
            profile = Some(load_profile_from_path(&app, path.clone())?);
        }
    }

    Ok(InitialSettingsLoadResponse { settings, profile })
}

#[tauri::command(async)]
pub fn load_settings() -> Settings {
    read_settings_or_default()
}

#[tauri::command(async)]
pub fn save_settings(app: AppHandle, mut settings: Settings) -> Result<(), String> {
    settings.most_recent_profile = get_latest_profile_path(&app);
    write_settings_to_file(&settings).map_err(|e| e.to_string())
}
