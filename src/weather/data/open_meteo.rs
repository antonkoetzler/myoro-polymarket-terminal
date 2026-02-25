//! Open-Meteo forecast client. No API key; free, 10k calls/day.

use anyhow::{Context, Result};
use serde::Deserialize;

const BASE: &str = "https://api.open-meteo.com/v1/forecast";

/// Daily forecast for one day (e.g. for temperature markets).
#[derive(Clone, Debug, Deserialize)]
pub struct DailyForecast {
    pub date: String,
    pub temperature_2m_max: Option<f64>,
    pub temperature_2m_min: Option<f64>,
}

#[derive(Deserialize)]
struct ApiResponse {
    daily: Option<Daily>,
}

#[derive(Deserialize)]
struct Daily {
    time: Vec<String>,
    temperature_2m_max: Option<Vec<Option<f64>>>,
    temperature_2m_min: Option<Vec<Option<f64>>>,
}

pub struct OpenMeteoClient {
    client: reqwest::blocking::Client,
}

impl OpenMeteoClient {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .build()
            .context("build HTTP client")?;
        Ok(Self { client })
    }

    /// Fetch daily forecast for the next 7 days at lat/lon.
    pub fn fetch_daily(&self, latitude: f64, longitude: f64) -> Result<Vec<DailyForecast>> {
        let url = format!(
            "{}?latitude={}&longitude={}&daily=temperature_2m_max,temperature_2m_min&timezone=auto",
            BASE, latitude, longitude
        );
        let resp: ApiResponse = self
            .client
            .get(&url)
            .send()
            .context("request Open-Meteo")?
            .error_for_status()
            .context("Open-Meteo status")?
            .json()
            .context("Open-Meteo JSON")?;

        let daily = resp.daily.context("missing daily")?;
        let n = daily.time.len();
        let maxs = daily.temperature_2m_max.as_deref().unwrap_or(&[]);
        let mins = daily.temperature_2m_min.as_deref().unwrap_or(&[]);

        let out = (0..n)
            .map(|i| DailyForecast {
                date: daily.time.get(i).cloned().unwrap_or_default(),
                temperature_2m_max: maxs.get(i).and_then(|x| *x),
                temperature_2m_min: mins.get(i).and_then(|x| *x),
            })
            .collect();
        Ok(out)
    }
}
