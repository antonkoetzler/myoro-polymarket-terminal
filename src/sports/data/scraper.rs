//! Fetch Premier League fixtures: FBRef (scrape) or Fixture Download (JSON fallback).

use anyhow::{Context, Result};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0";

/// One fixture or result: home, away, optional score, date.
#[derive(Clone, Debug, Serialize)]
pub struct Fixture {
    pub date: String,
    pub home: String,
    pub away: String,
    pub home_goals: Option<u8>,
    pub away_goals: Option<u8>,
}

const FBREF_PL_SCHEDULE: &str =
    "https://fbref.com/en/comps/9/schedule/Premier-League-Scores-and-Fixtures";
/// Fallback when FBRef returns 403 (Cloudflare).
const FIXTURE_DOWNLOAD_EPL: &str = "https://fixturedownload.com/feed/json/epl-2024";

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FixtureDownloadItem {
    date_utc: String,
    home_team: String,
    away_team: String,
    #[serde(default)]
    home_team_score: Option<i64>,
    #[serde(default)]
    away_team_score: Option<i64>,
}

pub struct SportsScraper {
    client: reqwest::blocking::Client,
}

impl SportsScraper {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("build HTTP client")?;
        Ok(Self { client })
    }

    /// Fetch Premier League fixtures. Tries Fixture Download JSON first (no rate limit); fallback FBRef.
    pub fn fetch_pl_fixtures(&self) -> Result<Vec<Fixture>> {
        if let Ok(fixtures) = self.fetch_pl_fixtures_fallback() {
            if !fixtures.is_empty() {
                return Ok(fixtures);
            }
        }
        let resp = self
            .client
            .get(FBREF_PL_SCHEDULE)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "en-US,en;q=0.5")
            .header("Referer", "https://fbref.com/")
            .send()
            .context("request FBRef schedule")?;
        let status = resp.status();
        let body = resp.text().context("FBRef schedule body")?;
        if status.is_success() {
            return parse_fbref_schedule(&body);
        }
        if status.as_u16() == 403 {
            return self.fetch_pl_fixtures_fallback();
        }
        anyhow::bail!(
            "FBRef schedule HTTP {}: {}",
            status,
            body.lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(80)
                .collect::<String>()
        );
    }

    fn fetch_pl_fixtures_fallback(&self) -> Result<Vec<Fixture>> {
        let body = self
            .client
            .get(FIXTURE_DOWNLOAD_EPL)
            .send()
            .context("request Fixture Download")?
            .error_for_status()
            .context("Fixture Download status")?
            .text()
            .context("Fixture Download body")?;
        let items: Vec<FixtureDownloadItem> =
            serde_json::from_str(&body).context("parse Fixture Download JSON")?;
        Ok(items
            .into_iter()
            .map(|i| {
                let date = i.date_utc.get(..10).unwrap_or(&i.date_utc).to_string();
                Fixture {
                    date,
                    home: i.home_team,
                    away: i.away_team,
                    home_goals: i.home_team_score.and_then(|n| u8::try_from(n).ok()),
                    away_goals: i.away_team_score.and_then(|n| u8::try_from(n).ok()),
                }
            })
            .collect())
    }
}

/// Parse FBRef schedule table. Table has thead + tbody; rows have data-stat="date", "home_team", etc.
fn parse_fbref_schedule(html: &str) -> Result<Vec<Fixture>> {
    let doc = Html::parse_document(html);
    let row_sel = Selector::parse("table.stats_table tbody tr")
        .map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let date_sel =
        Selector::parse("[data-stat=\"date\"]").map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let home_sel = Selector::parse("[data-stat=\"home_team\"]")
        .map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let away_sel = Selector::parse("[data-stat=\"away_team\"]")
        .map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let home_goals_sel = Selector::parse("[data-stat=\"home_goals\"]")
        .map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let away_goals_sel = Selector::parse("[data-stat=\"away_goals\"]")
        .map_err(|e| anyhow::anyhow!("selector: {}", e))?;

    let mut fixtures = Vec::new();
    for row in doc.select(&row_sel) {
        let date = row
            .select(&date_sel)
            .next()
            .and_then(|e| e.text().next())
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        let home = row
            .select(&home_sel)
            .next()
            .and_then(|e| e.text().next())
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        let away = row
            .select(&away_sel)
            .next()
            .and_then(|e| e.text().next())
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        let goals_home = row
            .select(&home_goals_sel)
            .next()
            .and_then(|e| e.text().next())
            .and_then(|s| s.trim().parse::<u8>().ok());
        let goals_away = row
            .select(&away_goals_sel)
            .next()
            .and_then(|e| e.text().next())
            .and_then(|s| s.trim().parse::<u8>().ok());

        if date.is_empty() && home.is_empty() && away.is_empty() {
            continue;
        }
        fixtures.push(Fixture {
            date,
            home,
            away,
            home_goals: goals_home,
            away_goals: goals_away,
        });
    }

    Ok(fixtures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fbref_schedule_empty_html_returns_empty() {
        let out = parse_fbref_schedule("<html><body></body></html>").unwrap();
        assert!(out.is_empty());
    }
}
