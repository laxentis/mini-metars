#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::awc::{MetarDto, Station};
use crate::profiles::read_profile_from_file;
use crate::settings::{
    get_appstate_settings, get_latest_profile_path, read_settings_or_default, set_appstate_settings,
};
use crate::state::{AppState, VatsimDataFetch};
use crate::update::check_for_updates;
use anyhow::anyhow;
use log::{debug, error, info, trace, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tauri::plugin::TauriPlugin;
use tauri::{Runtime, State, WebviewWindowBuilder};
use tauri_plugin_log::{Target, TargetKind};
use vatsim_utils::models::{Atis, V3ResponseData};

mod awc;
mod profiles;
mod settings;
mod state;
mod update;
mod utils;
mod window;

const MAIN_WINDOW_LABEL: &str = "main";

fn build_logger<R: Runtime>() -> TauriPlugin<R> {
    let builder =
        tauri_plugin_log::Builder::new()
            .clear_targets()
            .level(if cfg!(debug_assertions) {
                log::LevelFilter::Trace
            } else {
                log::LevelFilter::Debug
            });

    #[cfg(not(target_os = "windows"))]
    let builder = builder.target(Target::new(TargetKind::LogDir {
        file_name: Some("logs".to_string()),
    }));

    #[cfg(target_os = "windows")]
    let builder = match dirs::config_local_dir().map(|p| p.join("Mini METARs")) {
        Some(p) => builder.target(Target::new(TargetKind::Folder {
            path: p,
            file_name: Some("logs".to_string()),
        })),
        None => builder.target(Target::new(TargetKind::LogDir {
            file_name: Some("logs".to_string()),
        })),
    };

    builder.build()
}

fn main() {
    tauri::Builder::default()
        .plugin(build_logger())
        .manage(Arc::new(AppState::new()))
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            fetch_metar,
            lookup_station,
            get_atis,
            initialize_datafeed,
            profiles::load_profile,
            profiles::save_current_profile,
            profiles::save_profile_as,
            settings::load_settings,
            settings::load_settings_initial,
            settings::save_settings
        ])
        .setup(|app| {
            set_appstate_settings(app.handle(), read_settings_or_default());

            let mut window_builder = WebviewWindowBuilder::new(
                app,
                MAIN_WINDOW_LABEL,
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Mini METARs")
            .always_on_top(
                get_appstate_settings(app.handle())
                    .unwrap_or_default()
                    .always_on_top(),
            );

            let mut x_position = 0.0;
            let mut y_position = 0.0;
            let mut width = 250.0;

            #[cfg(target_os = "windows")]
            let mut height = 58.0;
            #[cfg(not(target_os = "windows"))]
            let mut height = 64.0;

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                debug!("Starting version update check");
                let res = check_for_updates(&handle).await;
                match res {
                    Ok(()) => {}
                    Err(e) => info!("Error while checking for updates: {e:?}"),
                }
            });

            if let Some(profile_path) = get_latest_profile_path(app.handle()) {
                debug!("Initialization - found latest profile path: {profile_path:?}");
                if let Ok(profile) = read_profile_from_file(profile_path.as_path()) {
                    debug!("Initialization - read latest profile: {profile:?}");
                    if let Some(window) = profile.window {
                        if let Some(position) = window.position {
                            x_position = f64::from(position.x) / window.scale_factor;
                            y_position = f64::from(position.y) / window.scale_factor;
                        }
                        if let Some(size) = window.size {
                            width = f64::from(size.width) / window.scale_factor;
                            height = f64::from(size.height) / window.scale_factor;
                        }
                    }
                }
            }

            window_builder = window_builder.inner_size(width, height);
            debug!("Initializing window size to width: {width}, height: {height}");

            if x_position != 0.0 || y_position != 0.0 {
                window_builder = window_builder.position(x_position, y_position);
                debug!("Initializing window position to x: {x_position}, y: {y_position}");
            }

            // Use custom titlebar on Windows only
            #[cfg(target_os = "windows")]
            let window_builder = window_builder.decorations(false);

            let _ = window_builder.build().unwrap();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchMetarResponse {
    metar: MetarDto,
    wind_string: String,
    altimeter: Altimeter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Copy)]
#[serde(rename_all = "camelCase")]
struct Altimeter {
    in_hg: f64,
    hpa: f64,
}

#[tauri::command]
async fn initialize_datafeed(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    debug!("Initializing VATSIM datafeed");
    let new_data = Some(VatsimDataFetch::new(fetch_vatsim_data(&state).await));
    *state.latest_vatsim_data.lock().unwrap() = new_data;
    Ok(())
}

#[tauri::command]
async fn fetch_metar(
    id: &str,
    state: State<'_, Arc<AppState>>,
) -> Result<FetchMetarResponse, String> {
    if let Ok(client) = &state.get_awc_client().await {
        let ret = client
            .fetch_metar(id)
            .await
            .map_err(|e| format!("Error fetching METAR for : {e:?}"))
            .map(|m| FetchMetarResponse {
                wind_string: m.wind_string(),
                altimeter: Altimeter {
                    in_hg: m.altimeter_in_hg(),
                    hpa: m.altimeter_hpa(),
                },
                metar: m,
            });

        match &ret {
            Ok(_m) => debug!("Successfully retrieved metar for {id}"),
            Err(e) => debug!("{e:?}"),
        }

        ret
    } else {
        const E: &str = "AWC Api Client not initialized";
        error!("Fetch Metar Command error: {E}",);
        Err(E.to_string())
    }
}

#[tauri::command]
async fn lookup_station(id: &str, state: State<'_, Arc<AppState>>) -> Result<Station, String> {
    debug!("Starting Lookup Station Command");
    if let Ok(client) = &state.get_awc_client().await {
        let ret = client
            .lookup_station(id)
            .map_err(|e| format!("Error looking up station {id}: {e:?}"));

        match &ret {
            Ok(s) => debug!("Lookup for {id} returned {s:?}"),
            Err(e) => debug!("Lookup for {id} returned {e}"),
        }

        ret
    } else {
        const E: &str = "AWC Api Client not initialized";
        error!("Fetch Metar Command error: {E}");
        Err(E.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchAtisResponse {
    pub letter: String,
    pub texts: Vec<String>,
}

#[tauri::command]
async fn get_atis(
    icao_id: &str,
    state: State<'_, Arc<AppState>>,
) -> Result<FetchAtisResponse, String> {
    if datafeed_is_stale(&state) {
        debug!("Datafeed is stale, fetching new data");
        let new_data = Some(VatsimDataFetch::new(fetch_vatsim_data(&state).await));
        *state.latest_vatsim_data.lock().unwrap() = new_data;
    }

    if let Some(fetch) = &*state.latest_vatsim_data.lock().unwrap() {
        fetch.data.as_ref().map_or_else(
            |_| Err("Could not retrieve datafeed".to_string()),
            |datafeed| {
                let found_atis: Vec<&Atis> = datafeed
                    .atis
                    .iter()
                    .filter(|a| a.callsign.starts_with(icao_id))
                    .collect();

                trace!(
                    "Found {} atis for {} with callsign(s): {:?}",
                    found_atis.len(),
                    icao_id,
                    found_atis
                        .iter()
                        .map(|a| &a.callsign)
                        .cloned()
                        .collect::<Vec<_>>()
                );

                let letter_str: String = match found_atis.len() {
                    0 => "-".to_string(),
                    1 => parse_atis_code(found_atis[0]),
                    _ => format!(
                        "{}/{}",
                        filter_callsign_and_parse(&found_atis, "_A_"),
                        filter_callsign_and_parse(&found_atis, "_D_")
                    ),
                };

                Ok(FetchAtisResponse {
                    letter: letter_str,
                    texts: found_atis
                        .iter()
                        .filter_map(|a| a.text_atis.as_ref().map(|t| t.join(" ")))
                        .collect(),
                })
            },
        )
    } else {
        const E: &str = "Could not retrieve datafeed";
        warn!("Get Atis Command error: {E}");
        Err(E.to_string())
    }
}

fn filter_callsign_and_parse(atises: &[&Atis], pat: &str) -> String {
    atises
        .iter()
        .find(|s| s.callsign.contains(pat))
        .map_or_else(|| "-".to_string(), |a| parse_atis_code(a))
}

fn parse_atis_code(atis: &Atis) -> String {
    match (&atis.atis_code, &atis.text_atis) {
        (Some(code), Some(text_lines)) => {
            // Check for special case that letter in ATIS text has advanced but `atis_code` field has not yet
            if let (Some(c), Some(text_c)) = (code.chars().next(), parse_code_from_text(text_lines))
            {
                trace!(
                    "Found letters for {}, code: {}, text parse:{}",
                    atis.callsign,
                    c,
                    text_c
                );
                match (text_c as i32) - (c as i32) {
                    1 => text_c.to_string(),
                    _ => c.to_string(),
                }
            } else {
                code.clone()
            }
        }
        (Some(code), None) => code.clone(),
        (None, Some(text_lines)) => {
            parse_code_from_text(text_lines).map_or_else(|| "-".to_string(), |c| c.to_string())
        }
        _ => "-".to_string(),
    }
}

fn parse_code_from_text(text_lines: &[String]) -> Option<char> {
    static INFO_CHAR_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?:INFO|INFORMATION) ([A-Z])(?:\W|$)").unwrap());
    static INFO_WORD_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?:INFO|INFORMATION) ([A-Z]+)(?:\W|$)").unwrap());

    let joined = text_lines.join(" ");
    INFO_CHAR_REGEX.captures(&joined).map_or_else(
        || {
            INFO_WORD_REGEX
                .captures(&joined)
                .and_then(|c| nato_to_char(&c[1]))
        },
        |c| c[1].chars().next(),
    )
}

fn nato_to_char(str: &str) -> Option<char> {
    match str.to_uppercase().as_str() {
        "ALPHA" => Some('A'),
        "BRAVO" => Some('B'),
        "CHARLIE" => Some('C'),
        "DELTA" => Some('D'),
        "ECHO" => Some('E'),
        "FOXTROT" => Some('F'),
        "GOLF" => Some('G'),
        "HOTEL" => Some('H'),
        "INDIA" => Some('I'),
        "JULIET" => Some('J'),
        "KILO" => Some('K'),
        "LIMA" => Some('L'),
        "MIKE" => Some('M'),
        "NOVEMBER" => Some('N'),
        "OSCAR" => Some('O'),
        "PAPA" => Some('P'),
        "QUEBEC" => Some('Q'),
        "ROMEO" => Some('R'),
        "SIERRA" => Some('S'),
        "TANGO" => Some('T'),
        "UNIFORM" => Some('U'),
        "VICTOR" => Some('V'),
        "WHISKEY" => Some('W'),
        "XRAY" => Some('X'),
        "X-RAY" => Some('X'),
        "YANKEE" => Some('Y'),
        "ZULU" => Some('Z'),
        _ => None,
    }
}

fn datafeed_is_stale(state: &State<'_, Arc<AppState>>) -> bool {
    state
        .latest_vatsim_data
        .lock()
        .unwrap()
        .as_ref()
        .map_or_else(
            || true,
            |fetch| fetch.fetched_time.elapsed() > Duration::from_secs(30),
        )
}

async fn fetch_vatsim_data(
    state: &State<'_, Arc<AppState>>,
) -> Result<V3ResponseData, anyhow::Error> {
    if let Ok(client) = state.get_vatsim_client().await {
        client.get_v3_data().await.map_err(Into::into)
    } else {
        const E: &str = "VATSIM API client not initialized";
        error!("Error fetching VATSIM data: {E}");
        Err(anyhow!(E))
    }
}
