//! FBRef team xG scraper — season-average attack/defence stats per team.

use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::collections::HashMap;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0";

/// Season-average stats for a single team. All values are per 90 minutes.
#[derive(Clone, Debug, Default)]
pub struct TeamXgStats {
    pub team: String,
    pub league: String,
    /// xG generated per 90 min (attack strength).
    pub xg_per_90: f64,
    /// xG conceded per 90 min (defensive weakness).
    pub xga_per_90: f64,
    /// Win rate in last 10 home games.
    pub home_win_rate: f64,
    /// Win rate in last 10 away games.
    pub away_win_rate: f64,
}

/// FBRef squad xG URL for the Premier League (comp 9).
const FBREF_XG_URL: &str = "https://fbref.com/en/comps/9/stats/Premier-League-Stats";

pub struct XgScraper {
    client: reqwest::blocking::Client,
}

impl XgScraper {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .context("build HTTP client")?;
        Ok(Self { client })
    }

    /// Fetch team xG stats for the Premier League.
    /// Returns a map of team name → stats. Falls back to empty map on error.
    pub fn fetch_pl_xg(&self) -> HashMap<String, TeamXgStats> {
        self.fetch_xg_from_url(FBREF_XG_URL, "EPL")
            .unwrap_or_default()
    }

    fn fetch_xg_from_url(&self, url: &str, league: &str) -> Result<HashMap<String, TeamXgStats>> {
        let body = self
            .client
            .get(url)
            .header("Accept", "text/html")
            .header("Accept-Language", "en-US,en;q=0.5")
            .header("Referer", "https://fbref.com/")
            .send()
            .context("xG request")?
            .text()
            .context("xG body")?;

        parse_fbref_xg(&body, league)
    }
}

fn parse_fbref_xg(html: &str, league: &str) -> Result<HashMap<String, TeamXgStats>> {
    let doc = Html::parse_document(html);
    let row_sel = Selector::parse("table#stats_squads_standard_for tbody tr")
        .map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let squad_sel =
        Selector::parse("[data-stat=\"squad\"]").map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let xg_sel =
        Selector::parse("[data-stat=\"xg\"]").map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let xga_sel =
        Selector::parse("[data-stat=\"xga\"]").map_err(|e| anyhow::anyhow!("selector: {}", e))?;
    let mp_sel =
        Selector::parse("[data-stat=\"games\"]").map_err(|e| anyhow::anyhow!("selector: {}", e))?;

    let mut map = HashMap::new();
    for row in doc.select(&row_sel) {
        let team = row
            .select(&squad_sel)
            .next()
            .and_then(|e| e.text().next())
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if team.is_empty() {
            continue;
        }
        let games = row
            .select(&mp_sel)
            .next()
            .and_then(|e| e.text().next())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(1.0)
            .max(1.0);
        let xg_total = row
            .select(&xg_sel)
            .next()
            .and_then(|e| e.text().next())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let xga_total = row
            .select(&xga_sel)
            .next()
            .and_then(|e| e.text().next())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);

        map.insert(
            team.clone(),
            TeamXgStats {
                team,
                league: league.to_string(),
                xg_per_90: xg_total / games,
                xga_per_90: xga_total / games,
                // Home/away win rates default to league average until we have per-game data.
                home_win_rate: 0.46,
                away_win_rate: 0.27,
            },
        );
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_html_returns_empty_map() {
        let result = parse_fbref_xg("<html><body></body></html>", "EPL").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn team_xg_stats_default_is_sane() {
        let stats = TeamXgStats::default();
        assert_eq!(stats.xg_per_90, 0.0);
        assert_eq!(stats.home_win_rate, 0.0);
    }
}
