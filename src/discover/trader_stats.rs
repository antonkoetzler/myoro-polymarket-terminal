//! Per-profile stats from Data API /trades: trade count (frequency), top category.

use serde::Deserialize;
use std::collections::HashMap;

const TRADES: &str = "https://data-api.polymarket.com/trades";
const TRADES_LIMIT: u32 = 500;

#[derive(Clone, Debug, Default)]
pub struct TraderStats {
    pub trade_count: u32,
    pub top_category: String,
}

#[derive(Debug, Deserialize)]
struct ApiTrade {
    #[serde(rename = "conditionId")]
    condition_id: Option<String>,
    #[serde(rename = "eventSlug")]
    event_slug: Option<String>,
    slug: Option<String>,
    size: Option<f64>,
    price: Option<f64>,
    side: Option<String>,
}

fn infer_category(slug: &str, event_slug: &str) -> &'static str {
    let s = format!("{} {}", slug.to_lowercase(), event_slug.to_lowercase());
    if s.contains("bitcoin") || s.contains("btc") || s.contains("crypto") || s.contains("eth") {
        "Crypto"
    } else if s.contains("trump")
        || s.contains("election")
        || s.contains("biden")
        || s.contains("politic")
    {
        "Politics"
    } else if s.contains("sport")
        || s.contains("nfl")
        || s.contains("nba")
        || s.contains("game")
        || s.contains("match")
    {
        "Sports"
    } else if s.contains("weather") || s.contains("temp") || s.contains("rain") {
        "Weather"
    } else if s.contains("fed")
        || s.contains("inflation")
        || s.contains("gdp")
        || s.contains("econom")
    {
        "Economics"
    } else if s.contains("tech") || s.contains("stock") || s.contains("nasdaq") {
        "Tech"
    } else if s.contains("finance") || s.contains("rate") {
        "Finance"
    } else {
        "Other"
    }
}

pub fn fetch_stats(address: &str) -> Option<TraderStats> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .ok()?;
    let mut trade_count: u32 = 0;
    let mut category_counts: HashMap<String, u32> = HashMap::new();
    let mut offset = 0u32;
    loop {
        let url = format!(
            "{}?user={}&limit=100&offset={}&takerOnly=false",
            TRADES, address, offset
        );
        let list: Vec<ApiTrade> = client.get(&url).send().ok()?.json().ok()?;
        if list.is_empty() {
            break;
        }
        for t in &list {
            trade_count += 1;
            let slug = t.event_slug.as_deref().unwrap_or("").to_string();
            let es = t.slug.as_deref().unwrap_or("");
            let cat = infer_category(es, &slug);
            *category_counts.entry(cat.to_string()).or_insert(0) += 1;
        }
        if list.len() < 100 {
            break;
        }
        offset += 100;
        if offset >= TRADES_LIMIT {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    let top_category = category_counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(k, _)| k)
        .unwrap_or_else(|| "—".to_string());
    Some(TraderStats {
        trade_count,
        top_category,
    })
}
