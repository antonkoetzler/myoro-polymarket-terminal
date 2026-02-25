//! Kalshi public REST API client — fetches soccer markets for cross-platform arb scanning.

use anyhow::{Context, Result};

const USER_AGENT: &str = "Mozilla/5.0 (compatible; polymarket-terminal/1.0)";
const KALSHI_MARKETS_URL: &str =
    "https://api.elections.kalshi.com/trade-api/v2/markets?limit=200&status=open&series_ticker=SOCCER";

/// A single Kalshi market snapshot.
#[derive(Clone, Debug)]
pub struct KalshiMarket {
    pub ticker: String,
    pub title: String,
    /// Kalshi YES price in the range 0.0–1.0.
    pub yes_price: f64,
    /// Kalshi NO price = 1.0 - yes_price (adjusted for fee spread).
    pub no_price: f64,
}

pub struct KalshiClient {
    client: reqwest::blocking::Client,
}

impl KalshiClient {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("build HTTP client")?;
        Ok(Self { client })
    }

    /// Fetch open soccer markets from Kalshi. Returns empty Vec on any error.
    pub fn fetch_soccer_markets(&self) -> Vec<KalshiMarket> {
        self.fetch_markets(KALSHI_MARKETS_URL).unwrap_or_default()
    }

    fn fetch_markets(&self, url: &str) -> Result<Vec<KalshiMarket>> {
        let body = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .context("Kalshi request")?
            .error_for_status()
            .context("Kalshi status")?
            .text()
            .context("Kalshi body")?;

        parse_kalshi_response(&body)
    }
}

fn parse_kalshi_response(json: &str) -> Result<Vec<KalshiMarket>> {
    let v: serde_json::Value = serde_json::from_str(json).context("parse JSON")?;
    let markets = v
        .get("markets")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();

    let result = markets
        .iter()
        .filter_map(|m| {
            let ticker = m.get("ticker").and_then(|t| t.as_str())?.to_string();
            let title = m
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or(&ticker)
                .to_string();

            // Kalshi prices are in cents (0–100). Normalize to 0.0–1.0.
            let yes_cents = m
                .get("yes_ask")
                .or_else(|| m.get("last_price"))
                .and_then(|p| p.as_f64())
                .unwrap_or(50.0);
            let yes_price = (yes_cents / 100.0).clamp(0.01, 0.99);
            let no_price = 1.0 - yes_price;

            Some(KalshiMarket {
                ticker,
                title,
                yes_price,
                no_price,
            })
        })
        .collect();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_markets_returns_empty() {
        let json = r#"{"markets":[]}"#;
        let result = parse_kalshi_response(json).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_valid_market() {
        let json = r#"{"markets":[{"ticker":"SOCCER-EPL-ARS-WIN","title":"Arsenal to win EPL match","yes_ask":60}]}"#;
        let result = parse_kalshi_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert!((result[0].yes_price - 0.60).abs() < 0.001);
        assert!((result[0].no_price - 0.40).abs() < 0.001);
    }

    #[test]
    fn price_clamped_to_valid_range() {
        let json = r#"{"markets":[{"ticker":"T1","title":"Test","yes_ask":0}]}"#;
        let result = parse_kalshi_response(json).unwrap();
        assert!(result[0].yes_price >= 0.01);
    }
}
