#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::awc::{AviationWeatherCenterApi, MetarDto, Station};
use anyhow::anyhow;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{State, WebviewWindowBuilder};
use tokio::sync::OnceCell;
use vatsim_utils::errors::VatsimUtilError;
use vatsim_utils::live_api::Vatsim;
use vatsim_utils::models::{Atis, V3ResponseData};

mod awc;
mod profiles;
mod settings;
mod utils;

const MAIN_WINDOW_LABEL: &str = "main";

pub struct VatsimDataFetch {
    pub fetched_time: Instant,
    pub data: Result<V3ResponseData, anyhow::Error>,
}

impl VatsimDataFetch {
    #[must_use]
    pub fn new(data: Result<V3ResponseData, anyhow::Error>) -> Self {
        Self {
            fetched_time: Instant::now(),
            data,
        }
    }
}

pub struct AppState {
    awc_client: OnceCell<Result<AviationWeatherCenterApi, anyhow::Error>>,
    vatsim_client: OnceCell<Result<Vatsim, VatsimUtilError>>,
    latest_vatsim_data: Mutex<Option<VatsimDataFetch>>,
    last_profile_path: Mutex<Option<PathBuf>>,
}

impl AppState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            awc_client: OnceCell::const_new(),
            vatsim_client: OnceCell::const_new(),
            latest_vatsim_data: Mutex::new(None),
            last_profile_path: Mutex::new(None),
        }
    }

    pub async fn get_awc_client(&self) -> &Result<AviationWeatherCenterApi, anyhow::Error> {
        self.awc_client
            .get_or_init(|| async { AviationWeatherCenterApi::try_new().await })
            .await
    }

    pub async fn get_vatsim_client(&self) -> &Result<Vatsim, VatsimUtilError> {
        self.vatsim_client
            .get_or_init(|| async { Vatsim::new().await })
            .await
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

type LockedState = Arc<AppState>;

fn main() {
    tauri::Builder::default()
        .manage(Arc::new(AppState::new()))
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            fetch_metar,
            lookup_station,
            get_atis,
            profiles::load_profile,
            profiles::save_current_profile,
            profiles::save_profile_as
        ])
        .setup(|app| {
            let window_builder = WebviewWindowBuilder::new(
                app,
                MAIN_WINDOW_LABEL,
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Mini METARs")
            .always_on_top(true);

            #[cfg(not(target_os = "windows"))]
            let window_builder = window_builder.inner_size(250.0, 64.0);

            // Use custom titlebar on Windows only
            #[cfg(target_os = "windows")]
            let window_builder = window_builder.inner_size(250.0, 58.0).decorations(false);

            let _ = window_builder.build().unwrap();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchMetarResponse {
    metar: MetarDto,
    wind_string: String,
    altimeter: f64,
}

#[tauri::command]
async fn fetch_metar(
    id: &str,
    state: State<'_, LockedState>,
) -> Result<FetchMetarResponse, String> {
    if let Ok(client) = &state.get_awc_client().await {
        client
            .fetch_metar(id)
            .await
            .map_err(|e| format!("Error fetching METARs: {e:?}"))
            .map(|m| FetchMetarResponse {
                wind_string: m.wind_string(),
                altimeter: m.altimeter_in_hg(),
                metar: m,
            })
    } else {
        Err("AWC Api Client not initialized".to_string())
    }
}

#[tauri::command]
async fn lookup_station(id: &str, state: State<'_, LockedState>) -> Result<Station, String> {
    if let Ok(client) = &state.get_awc_client().await {
        client
            .lookup_station(id)
            .map_err(|e| format!("Error looking up station {id}: {e:?}"))
    } else {
        Err("AWC Api Client not initialized".to_string())
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
    state: State<'_, LockedState>,
) -> Result<FetchAtisResponse, String> {
    if datafeed_is_stale(&state) {
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
        Err("Could not retrieve datafeed".to_string())
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
                match (text_c as u32) - (c as u32) {
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
    static INFO_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"INFO ([A-Z])").unwrap());
    static INFORMATION_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"INFORMATION ([A-Z])").unwrap());

    let joined = text_lines.join(" ");
    INFO_REGEX.captures(&joined).map_or_else(
        || {
            INFORMATION_REGEX
                .captures(&joined)
                .and_then(|c| c[1].chars().next())
        },
        |c| c[1].chars().next(),
    )
}

fn datafeed_is_stale(state: &State<'_, LockedState>) -> bool {
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
    state: &State<'_, LockedState>,
) -> Result<V3ResponseData, anyhow::Error> {
    if let Ok(client) = state.get_vatsim_client().await {
        client.get_v3_data().await.map_err(Into::into)
    } else {
        Err(anyhow!("VATSIM API client not initialized".to_string()))
    }
}
