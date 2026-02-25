//! Discover Polymarket profiles via Data API leaderboard. Background scan enriches with trade count + category.

pub mod trader_stats;

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use self::trader_stats::TraderStats;

const LEADERBOARD: &str = "https://data-api.polymarket.com/v1/leaderboard";
const PAGE_SIZE: u32 = 50;
const MAX_OFFSET: u32 = 1000;

#[derive(Clone, Debug)]
pub struct LeaderboardEntry {
    pub rank: String,
    pub proxy_wallet: String,
    pub user_name: String,
    pub vol: f64,
    pub pnl: f64,
}

#[derive(Debug, Deserialize)]
struct ApiEntry {
    rank: Option<String>,
    #[serde(rename = "proxyWallet")]
    proxy_wallet: Option<String>,
    #[serde(rename = "userName")]
    user_name: Option<String>,
    vol: Option<f64>,
    pnl: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::upper_case_acronyms)] // Polymarket API expects OVERALL, CRYPTO, etc.
pub enum LeaderboardCategory {
    #[default]
    OVERALL,
    CRYPTO,
    SPORTS,
    POLITICS,
    CULTURE,
    WEATHER,
    ECONOMICS,
    TECH,
    FINANCE,
}

#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::upper_case_acronyms)] // API expects DAY, WEEK, etc.
pub enum TimePeriod {
    DAY,
    #[default]
    WEEK,
    MONTH,
    ALL,
}

#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::upper_case_acronyms)] // API expects PNL, VOL
pub enum OrderBy {
    #[default]
    PNL,
    VOL,
}

pub struct DiscoverState {
    pub entries: RwLock<Vec<LeaderboardEntry>>,
    pub category: RwLock<LeaderboardCategory>,
    pub time_period: RwLock<TimePeriod>,
    pub order_by: RwLock<OrderBy>,
    pub stats_cache: RwLock<HashMap<String, TraderStats>>,
    pub scan_index: RwLock<usize>,
    pub fetching: AtomicBool,
}

impl Default for DiscoverState {
    fn default() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            category: RwLock::new(LeaderboardCategory::default()),
            time_period: RwLock::new(TimePeriod::default()),
            order_by: RwLock::new(OrderBy::default()),
            stats_cache: RwLock::new(HashMap::new()),
            scan_index: RwLock::new(0),
            fetching: AtomicBool::new(false),
        }
    }
}

impl DiscoverState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fetch(&self) {
        self.fetching.store(true, Ordering::SeqCst);
        if let Ok(mut g) = self.entries.write() {
            *g = Vec::new();
        }
        let cat = self
            .category
            .read()
            .ok()
            .map(|c| format!("{:?}", c))
            .unwrap_or_else(|| "OVERALL".to_string());
        let period = self
            .time_period
            .read()
            .ok()
            .map(|p| format!("{:?}", p))
            .unwrap_or_else(|| "WEEK".to_string());
        let order = self
            .order_by
            .read()
            .ok()
            .map(|o| format!("{:?}", o))
            .unwrap_or_else(|| "PNL".to_string());
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut all_entries: Vec<LeaderboardEntry> = Vec::new();
        let mut offset = 0u32;
        while offset <= MAX_OFFSET {
            let url = format!(
                "{}?category={}&timePeriod={}&orderBy={}&limit={}&offset={}",
                LEADERBOARD, cat, period, order, PAGE_SIZE, offset
            );
            let list: Vec<ApiEntry> = match client.get(&url).send().and_then(|r| r.json()) {
                Ok(l) => l,
                Err(_) => break,
            };
            let len = list.len();
            if len == 0 {
                break;
            }
            for e in list {
                let proxy_wallet = match &e.proxy_wallet {
                    Some(s) if s.starts_with("0x") && s.len() >= 42 => s.clone(),
                    _ => continue,
                };
                all_entries.push(LeaderboardEntry {
                    rank: e.rank.unwrap_or_else(|| "—".to_string()),
                    proxy_wallet,
                    user_name: e.user_name.unwrap_or_else(|| "—".to_string()),
                    vol: e.vol.unwrap_or(0.0),
                    pnl: e.pnl.unwrap_or(0.0),
                });
            }
            if len < PAGE_SIZE as usize {
                break;
            }
            offset += PAGE_SIZE;
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if let Ok(mut g) = self.entries.write() {
            *g = all_entries;
        }
        if let Ok(mut idx) = self.scan_index.write() {
            *idx = 0;
        }
        self.fetching.store(false, Ordering::SeqCst);
    }

    pub fn get_entries(&self) -> Vec<LeaderboardEntry> {
        self.entries.read().map(|e| e.clone()).unwrap_or_default()
    }

    pub fn is_fetching(&self) -> bool {
        self.fetching.load(Ordering::SeqCst)
    }

    pub fn cycle_category(&self) {
        use LeaderboardCategory::*;
        if let Ok(mut c) = self.category.write() {
            *c = match *c {
                OVERALL => CRYPTO,
                CRYPTO => SPORTS,
                SPORTS => POLITICS,
                POLITICS => CULTURE,
                CULTURE => WEATHER,
                WEATHER => ECONOMICS,
                ECONOMICS => TECH,
                TECH => FINANCE,
                FINANCE => OVERALL,
            };
        }
    }

    pub fn cycle_time_period(&self) {
        use TimePeriod::*;
        if let Ok(mut p) = self.time_period.write() {
            *p = match *p {
                DAY => WEEK,
                WEEK => MONTH,
                MONTH => ALL,
                ALL => DAY,
            };
        }
    }

    pub fn cycle_order_by(&self) {
        use OrderBy::*;
        if let Ok(mut o) = self.order_by.write() {
            *o = match *o {
                PNL => VOL,
                VOL => PNL,
            };
        }
    }

    pub fn category_label(&self) -> String {
        use LeaderboardCategory::*;
        self.category
            .read()
            .ok()
            .map(|c| match *c {
                OVERALL => "ALL".to_string(),
                other => format!("{:?}", other),
            })
            .unwrap_or_else(|| "ALL".to_string())
    }

    pub fn time_period_label(&self) -> String {
        self.time_period
            .read()
            .ok()
            .map(|p| format!("{:?}", p))
            .unwrap_or_else(|| "WEEK".to_string())
    }

    pub fn order_by_label(&self) -> String {
        use OrderBy::*;
        self.order_by
            .read()
            .ok()
            .map(|o| match *o {
                PNL => "P&L".to_string(),
                VOL => "VOL".to_string(),
            })
            .unwrap_or_else(|| "P&L".to_string())
    }

    pub fn category_index(&self) -> usize {
        use LeaderboardCategory::*;
        self.category
            .read()
            .ok()
            .map(|c| match *c {
                OVERALL => 0,
                CRYPTO => 1,
                SPORTS => 2,
                POLITICS => 3,
                CULTURE => 4,
                WEATHER => 5,
                ECONOMICS => 6,
                TECH => 7,
                FINANCE => 8,
            })
            .unwrap_or(0)
    }

    pub fn set_category_by_index(&self, i: usize) {
        use LeaderboardCategory::*;
        let c = match i % 9 {
            0 => OVERALL,
            1 => CRYPTO,
            2 => SPORTS,
            3 => POLITICS,
            4 => CULTURE,
            5 => WEATHER,
            6 => ECONOMICS,
            7 => TECH,
            _ => FINANCE,
        };
        if let Ok(mut g) = self.category.write() {
            *g = c;
        }
    }

    /// Index for dialog: 0=ALL, 1=DAY, 2=WEEK, 3=MONTH (ALL at top).
    pub fn time_period_index(&self) -> usize {
        use TimePeriod::*;
        self.time_period
            .read()
            .ok()
            .map(|p| match *p {
                ALL => 0,
                DAY => 1,
                WEEK => 2,
                MONTH => 3,
            })
            .unwrap_or(2)
    }

    pub fn set_time_period_by_index(&self, i: usize) {
        use TimePeriod::*;
        let p = match i % 4 {
            0 => ALL,
            1 => DAY,
            2 => WEEK,
            _ => MONTH,
        };
        if let Ok(mut g) = self.time_period.write() {
            *g = p;
        }
    }

    pub fn order_by_index(&self) -> usize {
        use OrderBy::*;
        self.order_by
            .read()
            .ok()
            .map(|o| match *o {
                PNL => 0,
                VOL => 1,
            })
            .unwrap_or(0)
    }

    pub fn set_order_by_index(&self, i: usize) {
        use OrderBy::*;
        let o = if i.is_multiple_of(2) { PNL } else { VOL };
        if let Ok(mut g) = self.order_by.write() {
            *g = o;
        }
    }

    pub fn get_stats(&self, address: &str) -> Option<TraderStats> {
        self.stats_cache.read().ok()?.get(address).cloned()
    }

    /// Called by background scanner: pick next leaderboard address, fetch /trades stats, cache. One profile per call.
    pub fn scan_next(&self) {
        let entries = self.get_entries();
        let len = entries.len();
        if len == 0 {
            return;
        }
        let mut idx = match self.scan_index.write() {
            Ok(g) => g,
            Err(_) => return,
        };
        let addr = entries[*idx].proxy_wallet.clone();
        *idx = (*idx + 1) % len;
        drop(idx);
        if let Some(stats) = self::trader_stats::fetch_stats(&addr) {
            if let Ok(mut cache) = self.stats_cache.write() {
                cache.insert(addr, stats);
            }
        }
    }
}
