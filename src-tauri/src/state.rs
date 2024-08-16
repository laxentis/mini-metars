use crate::awc::AviationWeatherCenterApi;
use crate::settings::Settings;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::OnceCell;
use vatsim_utils::errors::VatsimUtilError;
use vatsim_utils::live_api::Vatsim;
use vatsim_utils::models::V3ResponseData;

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
    pub latest_vatsim_data: Mutex<Option<VatsimDataFetch>>,
    pub settings: Mutex<Option<Settings>>,
}

impl AppState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            awc_client: OnceCell::const_new(),
            vatsim_client: OnceCell::const_new(),
            latest_vatsim_data: Mutex::new(None),
            settings: Mutex::new(None),
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
