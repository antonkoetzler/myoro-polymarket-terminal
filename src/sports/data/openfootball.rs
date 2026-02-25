//! OpenFootball GitHub CDN fixture client — no API key, no rate limiting.

use crate::sports::data::scraper::Fixture;
use anyhow::{Context, Result};
use serde::Deserialize;

const USER_AGENT: &str = "Mozilla/5.0 (compatible; polymarket-terminal/1.0)";

/// OpenFootball raw JSON fixture entry.
#[derive(Debug, Deserialize)]
struct OFMatch {
    #[serde(rename = "Date")]
    date: Option<String>,
    #[serde(rename = "Home Team")]
    home_team: Option<String>,
    #[serde(rename = "Away Team")]
    away_team: Option<String>,
    #[serde(rename = "HG")]
    hg: Option<serde_json::Value>,
    #[serde(rename = "AG")]
    ag: Option<serde_json::Value>,
}

/// Minimal wrapper around OpenFootball JSON file format.
#[derive(Debug, Deserialize)]
struct OFFile {
    #[serde(rename = "matches")]
    matches: Option<Vec<serde_json::Value>>,
}

/// Map short league name to OpenFootball raw GitHub URL.
fn league_url(short: &str, season: &str) -> Option<String> {
    let path = match short {
        "EPL" => format!("openfootball/england/master/2024-25/{}.json", season),
        "LaLiga" => format!("openfootball/espana/master/2024-25/{}.json", season),
        "Bundesliga" => format!("openfootball/bundesliga/master/2024-25/{}.json", season),
        "SerieA" => format!("openfootball/italy/master/2024-25/{}.json", season),
        "Ligue1" => format!("openfootball/france/master/2024-25/{}.json", season),
        _ => return None,
    };
    Some(format!("https://raw.githubusercontent.com/{}", path))
}

pub struct OpenFootballClient {
    client: reqwest::blocking::Client,
}

impl OpenFootballClient {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("build HTTP client")?;
        Ok(Self { client })
    }

    /// Fetch fixtures for a league (by short name, e.g. "EPL").
    /// Returns empty Vec on any error — caller should fall back to SportsScraper.
    pub fn fetch_fixtures(&self, league_short: &str) -> Vec<Fixture> {
        let url = match league_url(league_short, "1") {
            Some(u) => u,
            None => return Vec::new(),
        };
        self.fetch_url(&url).unwrap_or_default()
    }

    fn fetch_url(&self, url: &str) -> Result<Vec<Fixture>> {
        let body = self
            .client
            .get(url)
            .send()
            .context("OpenFootball request")?
            .error_for_status()
            .context("OpenFootball status")?
            .text()
            .context("OpenFootball body")?;

        let v: serde_json::Value = serde_json::from_str(&body).context("parse JSON")?;
        let matches = v
            .get("matches")
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default();

        let fixtures = matches
            .iter()
            .filter_map(|m| {
                let date = m
                    .get("Date")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let home = m
                    .get("Home Team")
                    .and_then(|h| h.as_str())
                    .unwrap_or("")
                    .to_string();
                let away = m
                    .get("Away Team")
                    .and_then(|a| a.as_str())
                    .unwrap_or("")
                    .to_string();
                if home.is_empty() || away.is_empty() {
                    return None;
                }
                let home_goals = m
                    .get("HG")
                    .and_then(|g| g.as_u64())
                    .and_then(|g| u8::try_from(g).ok());
                let away_goals = m
                    .get("AG")
                    .and_then(|g| g.as_u64())
                    .and_then(|g| u8::try_from(g).ok());
                Some(Fixture {
                    date,
                    home,
                    away,
                    home_goals,
                    away_goals,
                })
            })
            .collect();

        Ok(fixtures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn league_url_epl_returns_some() {
        assert!(league_url("EPL", "1").is_some());
    }

    #[test]
    fn league_url_unknown_returns_none() {
        assert!(league_url("UNKNOWN_LEAGUE", "1").is_none());
    }
}
