use crate::profiles::{load_profile_from_path, Profile};
use crate::state::AppState;
use crate::utils;
use crate::utils::deserialize_from_file;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    load_most_recent_profile_on_open: bool,
    most_recent_profile: Option<PathBuf>,
    always_on_top: bool,
    auto_resize: bool,
}

impl Settings {
    pub const fn new() -> Self {
        Self {
            load_most_recent_profile_on_open: true,
            most_recent_profile: None,
            always_on_top: true,
            auto_resize: true,
        }
    }

    pub const fn always_on_top(&self) -> bool {
        self.always_on_top
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

pub fn read_settings_or_default() -> Settings {
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

pub fn set_appstate_settings(app: &AppHandle, settings: Settings) {
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        *state.settings.lock().unwrap() = Some(settings);
    }
}

pub fn get_appstate_settings(app: &AppHandle) -> Option<Settings> {
    app.try_state::<Arc<AppState>>()
        .and_then(|state| state.settings.lock().unwrap().clone())
}

pub fn set_latest_profile_path(app: &AppHandle, path: PathBuf) {
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        let mut settings = state.settings.lock().unwrap();
        *settings = (*settings).as_ref().map_or_else(
            || Some(read_settings_or_default()),
            |s| {
                Some(Settings {
                    most_recent_profile: Some(path),
                    ..s.clone()
                })
            },
        );
    }
}

pub fn get_latest_profile_path(app: &AppHandle) -> Option<PathBuf> {
    app.try_state::<Arc<AppState>>().and_then(|state| {
        state
            .settings
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|s| s.most_recent_profile.clone())
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitialSettingsLoadResponse {
    pub settings: Settings,
    pub profile: Option<Profile>,
}

#[tauri::command(async)]
pub fn load_settings_initial(app: AppHandle) -> Result<InitialSettingsLoadResponse, String> {
    let settings = read_settings_or_default();
    set_appstate_settings(&app, settings.clone());

    let mut profile = None;
    if settings.load_most_recent_profile_on_open {
        if let Some(path) = settings.most_recent_profile.as_ref() {
            profile = Some(load_profile_from_path(&app, path.clone())?);
        }
    }

    Ok(InitialSettingsLoadResponse { settings, profile })
}

#[tauri::command(async)]
pub fn load_settings(app: AppHandle) -> Settings {
    let ret = read_settings_or_default();
    set_appstate_settings(&app, ret.clone());

    ret
}

#[tauri::command(async)]
pub fn save_settings(app: AppHandle, settings: Option<Settings>) -> Result<(), String> {
    let appstate_settings = get_appstate_settings(&app).unwrap_or_else(read_settings_or_default);
    let write_settings = settings.map_or(appstate_settings.clone(), |s| Settings {
        most_recent_profile: s
            .most_recent_profile
            .map_or_else(|| appstate_settings.most_recent_profile, Some),
        ..s
    });

    write_settings_to_file(&write_settings).map_err(|e| e.to_string())
}
