//! Live match scores via Sofascore unofficial API. Used by the 70-minute rule strategy.

use anyhow::{Context, Result};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0";
const SOFASCORE_BASE: &str = "https://api.sofascore.com/api/v1";

/// Status of a live or recent match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchStatus {
    Scheduled,
    InPlay,
    HalfTime,
    Finished,
    Postponed,
    Unknown,
}

impl MatchStatus {
    fn from_str(s: &str) -> Self {
        match s {
            "inprogress" | "1st half" | "2nd half" => MatchStatus::InPlay,
            "halftime" => MatchStatus::HalfTime,
            "finished" => MatchStatus::Finished,
            "postponed" | "cancelled" => MatchStatus::Postponed,
            "notstarted" => MatchStatus::Scheduled,
            _ => MatchStatus::Unknown,
        }
    }
}

/// Snapshot of a live match.
#[derive(Clone, Debug)]
pub struct LiveMatchState {
    pub match_id: String,
    pub home_team: String,
    pub away_team: String,
    pub home_goals: u8,
    pub away_goals: u8,
    /// Match minute (0–90+).
    pub minute: u8,
    pub status: MatchStatus,
}

pub struct LiveScoresClient {
    client: reqwest::blocking::Client,
}

impl LiveScoresClient {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("build HTTP client")?;
        Ok(Self { client })
    }

    /// Fetch today's live soccer matches from Sofascore.
    /// Returns empty Vec on any error (unofficial API may break).
    pub fn fetch_live(&self) -> Vec<LiveMatchState> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let url = format!(
            "{}/sport/football/scheduled-events/{}",
            SOFASCORE_BASE, today
        );
        self.fetch_url(&url).unwrap_or_default()
    }

    fn fetch_url(&self, url: &str) -> Result<Vec<LiveMatchState>> {
        let body = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .header("Referer", "https://www.sofascore.com/")
            .send()
            .context("live scores request")?
            .error_for_status()
            .context("live scores status")?
            .text()
            .context("live scores body")?;

        parse_sofascore_response(&body)
    }
}

fn parse_sofascore_response(json: &str) -> Result<Vec<LiveMatchState>> {
    let v: serde_json::Value = serde_json::from_str(json).context("parse JSON")?;
    let events = v
        .get("events")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    let states = events
        .iter()
        .filter_map(|e| {
            let status_str = e
                .get("status")
                .and_then(|s| s.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");
            let status = MatchStatus::from_str(status_str);

            // Only include matches that are currently in play or half-time.
            if !matches!(status, MatchStatus::InPlay | MatchStatus::HalfTime) {
                return None;
            }

            let id = e
                .get("id")
                .and_then(|i| i.as_u64())
                .map(|i| i.to_string())?;
            let home = e
                .get("homeTeam")
                .and_then(|t| t.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let away = e
                .get("awayTeam")
                .and_then(|t| t.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            if home.is_empty() || away.is_empty() {
                return None;
            }
            let home_goals = e
                .get("homeScore")
                .and_then(|s| s.get("current"))
                .and_then(|g| g.as_u64())
                .and_then(|g| u8::try_from(g).ok())
                .unwrap_or(0);
            let away_goals = e
                .get("awayScore")
                .and_then(|s| s.get("current"))
                .and_then(|g| g.as_u64())
                .and_then(|g| u8::try_from(g).ok())
                .unwrap_or(0);
            let minute = e
                .get("time")
                .and_then(|t| t.get("played"))
                .and_then(|m| m.as_u64())
                .and_then(|m| u8::try_from(m).ok())
                .unwrap_or(0);

            Some(LiveMatchState {
                match_id: id,
                home_team: home,
                away_team: away,
                home_goals,
                away_goals,
                minute,
                status,
            })
        })
        .collect();

    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_events_returns_empty() {
        let json = r#"{"events":[]}"#;
        let result = parse_sofascore_response(json).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn non_live_events_filtered_out() {
        let json = r#"{"events":[{"id":1,"status":{"type":"notstarted"},"homeTeam":{"name":"Team A"},"awayTeam":{"name":"Team B"}}]}"#;
        let result = parse_sofascore_response(json).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn match_status_from_str() {
        assert_eq!(MatchStatus::from_str("inprogress"), MatchStatus::InPlay);
        assert_eq!(MatchStatus::from_str("finished"), MatchStatus::Finished);
        assert_eq!(MatchStatus::from_str("notstarted"), MatchStatus::Scheduled);
    }
}
