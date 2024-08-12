use crate::{utils, LockedState, MAIN_WINDOW_LABEL};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Wry};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder};

#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub stations: Vec<String>,
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

//
// #[derive(Debug, Serialize, Deserialize)]
// pub struct ProfileResponse {
//     pub filename: String,
//     pub directory: String,
//     pub data: Profile,
// }

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

fn get_latest_profile_path(app: &AppHandle) -> Option<PathBuf> {
    app.try_state::<LockedState>()
        .and_then(|state| state.last_profile_path.lock().unwrap().clone())
}

#[tauri::command(async)]
pub fn load_profile(app: AppHandle) -> Result<Profile, String> {
    let pick_response = profile_dialog_builder(&app).blocking_pick_file();
    pick_response.map_or_else(
        || Err("Could not pick file".to_string()),
        |pick| match read_profile_from_file(&pick.path) {
            Ok(profile) => {
                set_latest_profile_path(&app, pick.path);
                Ok(profile)
            }
            Err(e) => Err(e.to_string()),
        },
    )
}

#[tauri::command(async)]
pub fn save_current_profile(mut profile: Profile, app: AppHandle) -> Result<(), String> {
    println!("here");
    profile.window = get_window_state(&app);
    println!("{:?}", profile);
    app.try_state::<LockedState>().map_or_else(
        || Err("Could not get app state".to_string()),
        |state| {
            if let Some(path) = &mut *state.last_profile_path.lock().unwrap() {
                save_profile(&profile, path.clone(), &app)
            } else {
                save_profile_as(profile, app.clone())
            }
        },
    )
}

#[tauri::command(async)]
pub fn save_profile_as(mut profile: Profile, app: AppHandle) -> Result<(), String> {
    println!("here2");
    profile.window = get_window_state(&app);
    profile_dialog_builder(&app)
        .blocking_save_file()
        .map_or_else(
            || Err("Dialog closed without selecting save path".to_string()),
            |path| save_profile(&profile, path, &app),
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
    app.get_window(MAIN_WINDOW_LABEL)
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
    app.get_window(MAIN_WINDOW_LABEL).map_or_else(
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
