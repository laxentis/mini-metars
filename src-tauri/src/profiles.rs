use crate::{utils, LockedState, MAIN_WINDOW_LABEL};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, Wry};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub stations: Vec<String>,
    pub show_input: Option<bool>,
    pub show_titlebar: Option<bool>,
    pub window: Option<ProfileWindowState>,
}

#[derive(Debug, Serialize, Deserialize)]
enum WindowState {
    Maximized,
    FullScreen,
    Normal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileWindowState {
    state: WindowState,
    position: Option<PhysicalPosition<i32>>,
    size: Option<PhysicalSize<u32>>,
}

fn profiles_path() -> Option<PathBuf> {
    dirs::config_local_dir().map(|p| p.join("Mini METARs").join("Profiles"))
}

fn get_or_create_profiles_path() -> Option<PathBuf> {
    profiles_path().and_then(|p| utils::get_or_create_path(&p))
}

fn read_profile_from_file(path: &Path) -> Result<Profile, anyhow::Error> {
    utils::deserialize_from_file(path)
}

fn write_profile_to_file(path: &Path, profile: &Profile) -> Result<(), anyhow::Error> {
    utils::serialize_to_file(path, profile)
}

fn profile_dialog_builder(app: &AppHandle) -> FileDialogBuilder<Wry> {
    let mut builder = app.dialog().file().add_filter("Profile JSON", &["json"]);

    let latest_profile = get_latest_profile_path(app);
    let latest_profile_dir = latest_profile
        .as_ref()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let latest_profile_filename = latest_profile.as_ref().and_then(|p| p.file_name());

    let dialog_path =
        latest_profile_dir.map_or_else(get_or_create_profiles_path, |p| match p.try_exists() {
            Ok(true) => Some(p),
            _ => None,
        });
    if dialog_path.is_some() {
        builder = builder.set_directory(dialog_path.unwrap());
    }

    if latest_profile_filename.is_some() {
        builder = builder.set_file_name(
            latest_profile_filename
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap_or_default(),
        );
    }

    builder
}

fn set_latest_profile_path(app: &AppHandle, path: PathBuf) {
    if let Some(state) = app.try_state::<LockedState>() {
        *state.last_profile_path.lock().unwrap() = Some(path);
    }
}

pub fn get_latest_profile_path(app: &AppHandle) -> Option<PathBuf> {
    app.try_state::<LockedState>()
        .and_then(|state| state.last_profile_path.lock().unwrap().clone())
}

#[tauri::command(async)]
pub fn load_profile(app: AppHandle) -> Result<Profile, String> {
    let window = app.get_webview_window(MAIN_WINDOW_LABEL);
    set_always_on_top(window.as_ref(), false)?;
    let pick_response = profile_dialog_builder(&app).blocking_pick_file();
    let ret = pick_response.map_or_else(
        || Err("Could not pick file".to_string()),
        |pick| load_profile_from_path(&app, pick.path),
    );
    set_always_on_top(window.as_ref(), true)?;

    ret
}

pub fn load_profile_from_path(app: &AppHandle, path: PathBuf) -> Result<Profile, String> {
    match read_profile_from_file(&path) {
        Ok(profile) => {
            set_latest_profile_path(app, path.clone());
            if let Some(window) = &profile.window {
                apply_window_state(app, window).map_err(|e| e.to_string())?;
            }
            Ok(profile)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command(async)]
pub fn save_current_profile(mut profile: Profile, app: AppHandle) -> Result<(), String> {
    profile.window = get_window_state(&app);
    app.try_state::<LockedState>().map_or_else(
        || Err("Could not get app state".to_string()),
        |state| {
            let last_profile_path = state.last_profile_path.lock().unwrap().clone();
            if let Some(path) = last_profile_path {
                save_profile(&profile, path, &app)
            } else {
                save_profile_as(profile, app.clone())
            }
        },
    )
}

#[tauri::command(async)]
pub fn save_profile_as(mut profile: Profile, app: AppHandle) -> Result<(), String> {
    profile.window = get_window_state(&app);
    let window = app.get_webview_window(MAIN_WINDOW_LABEL);
    set_always_on_top(window.as_ref(), false)?;
    let ret = profile_dialog_builder(&app)
        .blocking_save_file()
        .map_or_else(
            || Err("Dialog closed without selecting save path".to_string()),
            |path| save_profile(&profile, path, &app),
        );
    set_always_on_top(window.as_ref(), true)?;

    ret
}

fn set_always_on_top(window: Option<&WebviewWindow>, always_on_top: bool) -> Result<(), String> {
    window.map_or_else(
        || Err("Could not find window".to_string()),
        |w| {
            w.set_always_on_top(always_on_top)
                .map_err(|e| e.to_string())
        },
    )
}

fn save_profile(profile: &Profile, path: PathBuf, app: &AppHandle) -> Result<(), String> {
    match write_profile_to_file(&path, profile) {
        Ok(()) => {
            set_latest_profile_path(app, path);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn get_window_state(app: &AppHandle) -> Option<ProfileWindowState> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .map(|w| ProfileWindowState {
            state: if w.is_maximized().unwrap_or_default() {
                WindowState::Maximized
            } else if w.is_fullscreen().unwrap_or_default() {
                WindowState::FullScreen
            } else {
                WindowState::Normal
            },
            position: w.outer_position().ok(),
            size: w.outer_size().ok(),
        })
}

fn apply_window_state(
    app: &AppHandle,
    window_state: &ProfileWindowState,
) -> Result<(), anyhow::Error> {
    app.get_webview_window(MAIN_WINDOW_LABEL).map_or_else(
        || Err(anyhow!("Could not find main window")),
        |w| {
            match window_state.state {
                WindowState::FullScreen => w.set_fullscreen(true)?,
                WindowState::Maximized => w.maximize()?,
                WindowState::Normal => {
                    if let Some(size) = window_state.size {
                        w.set_size(size)?;
                    }
                    if let Some(position) = window_state.position {
                        w.set_position(position)?;
                    }
                }
            }
            Ok(())
        },
    )
}
