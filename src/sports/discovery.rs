//! Match fixtures → Polymarket markets via Gamma API keyword search.

use crate::sports::data::Fixture;
use anyhow::{Context, Result};

const USER_AGENT: &str = "Mozilla/5.0 (compatible; polymarket-terminal/1.0)";
const GAMMA_EVENTS: &str = "https://gamma-api.polymarket.com/events?closed=false&limit=100";

/// A matched Polymarket market for a given fixture.
#[derive(Clone, Debug)]
pub struct PolymarketMarket {
    pub condition_id: String,
    pub asset_id: String,
    /// Implied probability for YES outcome (home team win in a match-winner market).
    pub yes_price: f64,
    /// Implied probability for NO outcome.
    pub no_price: f64,
    pub title: String,
}

/// Fixture enriched with xG stats and a matched Polymarket market (if found).
#[derive(Clone, Debug)]
pub struct FixtureWithStats {
    pub fixture: Fixture,
    pub home_xg_per_90: f64,
    pub home_xga_per_90: f64,
    pub away_xg_per_90: f64,
    pub away_xga_per_90: f64,
    pub home_win_rate: f64,
    pub away_win_rate: f64,
    pub polymarket: Option<PolymarketMarket>,
}

impl FixtureWithStats {
    pub fn from_fixture(fixture: Fixture) -> Self {
        Self {
            fixture,
            home_xg_per_90: 1.4,
            home_xga_per_90: 1.2,
            away_xg_per_90: 1.2,
            away_xga_per_90: 1.4,
            home_win_rate: 0.46,
            away_win_rate: 0.27,
            polymarket: None,
        }
    }
}

pub struct MarketDiscovery {
    client: reqwest::blocking::Client,
}

impl MarketDiscovery {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("build HTTP client")?;
        Ok(Self { client })
    }

    /// Match a fixture to a Polymarket market by team name keyword search.
    /// Returns None if no match is found or request fails.
    pub fn find_market(&self, fixture: &Fixture) -> Option<PolymarketMarket> {
        // Search using home team name as keyword.
        let keyword = urlencoding(&fixture.home);
        let url = format!("{}&q={}", GAMMA_EVENTS, keyword);

        let body = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .ok()?
            .error_for_status()
            .ok()?
            .text()
            .ok()?;

        let events: Vec<serde_json::Value> = serde_json::from_str(&body).ok()?;
        self.match_fixture_to_event(fixture, &events)
    }

    fn match_fixture_to_event(
        &self,
        fixture: &Fixture,
        events: &[serde_json::Value],
    ) -> Option<PolymarketMarket> {
        let home_lc = fixture.home.to_lowercase();
        let away_lc = fixture.away.to_lowercase();

        for event in events {
            let title = event
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_lowercase();

            // Require both team names to appear in the event title.
            if !title.contains(&home_lc) || !title.contains(&away_lc) {
                continue;
            }

            // Extract first market from event.
            let markets = event.get("markets").and_then(|m| m.as_array())?;
            let market = markets.first()?;

            let condition_id = market
                .get("conditionId")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            let asset_id = market
                .get("clobTokenIds")
                .and_then(|t| t.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let yes_price = market
                .get("outcomePrices")
                .and_then(|p| p.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.5)
                .clamp(0.01, 0.99);

            return Some(PolymarketMarket {
                condition_id,
                asset_id,
                yes_price,
                no_price: 1.0 - yes_price,
                title: event
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
        None
    }
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                c.to_string()
            } else if c == ' ' {
                "%20".to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_spaces() {
        assert_eq!(urlencoding("Man City"), "Man%20City");
    }

    #[test]
    fn fixture_with_stats_defaults() {
        let f = Fixture {
            date: "2025-03-01".to_string(),
            home: "Arsenal".to_string(),
            away: "Chelsea".to_string(),
            home_goals: None,
            away_goals: None,
        };
        let fs = FixtureWithStats::from_fixture(f);
        assert!(fs.polymarket.is_none());
        assert!(fs.home_xg_per_90 > 0.0);
    }

    #[test]
    fn match_fixture_no_events_returns_none() {
        let discovery = MarketDiscovery {
            client: reqwest::blocking::Client::new(),
        };
        let fixture = Fixture {
            date: "2025-03-01".to_string(),
            home: "Arsenal".to_string(),
            away: "Chelsea".to_string(),
            home_goals: None,
            away_goals: None,
        };
        assert!(discovery.match_fixture_to_event(&fixture, &[]).is_none());
    }
}
