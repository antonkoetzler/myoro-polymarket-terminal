//! Live data: Crypto (Gamma + Binance), Sports, Weather. Fetched in background; TUI reads.

use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

const GAMMA_EVENTS: &str = "https://gamma-api.polymarket.com/events?closed=false&limit=15";
const BINANCE_TICKER: &str = "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT";

#[derive(Default)]
pub struct CryptoState {
    pub btc_usdt: String,
    pub events: Vec<String>,
}

/// Embedded leagues.json — static list of leagues and teams.
const LEAGUES_JSON: &str = include_str!("../sports/leagues.json");

/// A league entry from leagues.json.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct League {
    pub name: String,
    pub short: String,
    pub country: String,
    pub tier: u32,
    pub teams: Vec<String>,
}

impl League {
    /// Load from the embedded JSON. Returns empty Vec on parse error.
    pub fn load_all() -> Vec<League> {
        serde_json::from_str(LEAGUES_JSON).unwrap_or_default()
    }
}

/// Configuration for a single sports strategy (toggle state, auto-execute).
#[derive(Clone, Debug)]
pub struct StrategyConfig {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub enabled: bool,
    pub auto_execute: bool,
    pub is_custom: bool,
}

impl StrategyConfig {
    pub fn builtins() -> Vec<StrategyConfig> {
        vec![
            StrategyConfig {
                id: "poisson",
                name: "Poisson Model",
                description: "Poisson + Dixon-Coles. Min edge 5%.",
                enabled: false,
                auto_execute: false,
                is_custom: false,
            },
            StrategyConfig {
                id: "home_adv",
                name: "Home Advantage",
                description: "Elo-adjusted +10% home uplift vs market.",
                enabled: false,
                auto_execute: false,
                is_custom: false,
            },
            StrategyConfig {
                id: "rule_1_20",
                name: "1.20 Rule",
                description: "Value on heavy favs (mkt 0.80-0.87). Min $5 Kelly.",
                enabled: false,
                auto_execute: false,
                is_custom: false,
            },
            StrategyConfig {
                id: "arb_scanner",
                name: "Cross-Platform Arb",
                description: "Poly vs Kalshi price discrepancy detection.",
                enabled: false,
                auto_execute: false,
                is_custom: false,
            },
            StrategyConfig {
                id: "in_play_70min",
                name: "70-Min Tie Rule",
                description: "Late-game xG value for losing team at 65-85 min.",
                enabled: false,
                auto_execute: false,
                is_custom: false,
            },
        ]
    }
}

/// A sports signal as stored in SportsState.
#[derive(Clone, Debug)]
pub struct StoredSignal {
    pub market_id: String,
    pub home: String,
    pub away: String,
    pub date: String,
    pub side: String,
    pub edge_pct: f64,
    pub kelly_size: f64,
    pub strategy_id: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A live match state snapshot for the 70-min rule.
#[derive(Clone, Debug)]
pub struct LiveMatchSnapshot {
    pub home_team: String,
    pub away_team: String,
    pub home_goals: u8,
    pub away_goals: u8,
    pub minute: u8,
}

/// Expanded sports tab state.
pub struct SportsState {
    pub fixtures: Vec<crate::sports::FixtureWithStats>,
    pub signals: Vec<StoredSignal>,
    pub live_matches: Vec<LiveMatchSnapshot>,
    pub xg_cache: HashMap<String, crate::sports::data::TeamXgStats>,
    pub strategy_configs: Vec<StrategyConfig>,
    pub leagues: Vec<League>,
}

impl Default for SportsState {
    fn default() -> Self {
        Self {
            fixtures: Vec::new(),
            signals: Vec::new(),
            live_matches: Vec::new(),
            xg_cache: HashMap::new(),
            strategy_configs: StrategyConfig::builtins(),
            leagues: League::load_all(),
        }
    }
}

#[derive(Default)]
pub struct WeatherState {
    pub forecast: Vec<String>,
}

const MAX_LOGS: usize = 80;

fn short_ts() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

fn truncate_log(g: &mut Vec<(LogLevel, String)>) {
    let drop = g.len().saturating_sub(MAX_LOGS);
    if drop > 0 {
        g.drain(0..drop);
    }
}

/// Global stats shown on every tab.
pub struct GlobalStats {
    pub bankroll: Option<f64>,
    pub pnl: f64,
    pub open_trades: u32,
    pub closed_trades: u32,
}

impl Default for GlobalStats {
    fn default() -> Self {
        Self {
            bankroll: None,
            pnl: 0.0,
            open_trades: 0,
            closed_trades: 0,
        }
    }
}

pub struct LiveState {
    pub crypto: RwLock<CryptoState>,
    pub sports: RwLock<SportsState>,
    pub weather: RwLock<WeatherState>,
    pub crypto_logs: RwLock<Vec<(LogLevel, String)>>,
    pub sports_logs: RwLock<Vec<(LogLevel, String)>>,
    pub weather_logs: RwLock<Vec<(LogLevel, String)>>,
    pub copy_logs: RwLock<Vec<(LogLevel, String)>>,
    pub discover_logs: RwLock<Vec<(LogLevel, String)>>,
    pub global_stats: RwLock<GlobalStats>,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
            crypto: RwLock::new(CryptoState::default()),
            sports: RwLock::new(SportsState::default()),
            weather: RwLock::new(WeatherState::default()),
            crypto_logs: RwLock::new(Vec::new()),
            sports_logs: RwLock::new(Vec::new()),
            weather_logs: RwLock::new(Vec::new()),
            copy_logs: RwLock::new(Vec::new()),
            discover_logs: RwLock::new(Vec::new()),
            global_stats: RwLock::new(GlobalStats::default()),
        }
    }
}

impl LiveState {
    pub fn push_log(&self, s: String) {
        self.push_copy_log(LogLevel::Info, s);
    }

    pub fn push_crypto_log(&self, level: LogLevel, s: String) {
        if let Ok(mut g) = self.crypto_logs.write() {
            g.push((level, format!("{} {}", short_ts(), s)));
            truncate_log(&mut g);
        }
    }
    pub fn push_sports_log(&self, level: LogLevel, s: String) {
        if let Ok(mut g) = self.sports_logs.write() {
            g.push((level, format!("{} {}", short_ts(), s)));
            truncate_log(&mut g);
        }
    }
    pub fn push_weather_log(&self, level: LogLevel, s: String) {
        if let Ok(mut g) = self.weather_logs.write() {
            g.push((level, format!("{} {}", short_ts(), s)));
            truncate_log(&mut g);
        }
    }
    pub fn push_copy_log(&self, level: LogLevel, s: String) {
        if let Ok(mut g) = self.copy_logs.write() {
            g.push((level, format!("{} {}", short_ts(), s)));
            truncate_log(&mut g);
        }
    }

    pub fn get_crypto_logs(&self) -> Vec<(LogLevel, String)> {
        self.crypto_logs
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
    pub fn get_sports_logs(&self) -> Vec<(LogLevel, String)> {
        self.sports_logs
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
    pub fn get_weather_logs(&self) -> Vec<(LogLevel, String)> {
        self.weather_logs
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
    pub fn get_copy_logs(&self) -> Vec<(LogLevel, String)> {
        self.copy_logs.read().map(|g| g.clone()).unwrap_or_default()
    }
    pub fn get_discover_logs(&self) -> Vec<(LogLevel, String)> {
        self.discover_logs
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn last_log_is_error(&self, tab: u8) -> bool {
        let logs = match tab {
            0 => self.get_crypto_logs(),
            1 => self.get_sports_logs(),
            2 => self.get_weather_logs(),
            _ => return false,
        };
        logs.last()
            .map(|(l, _)| *l == LogLevel::Error)
            .unwrap_or(false)
    }

    pub fn set_bankroll(&self, v: Option<f64>) {
        if let Ok(mut s) = self.global_stats.write() {
            s.bankroll = v;
        }
    }
}

impl LiveState {
    pub fn fetch_all(&self) {
        if let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            self.push_crypto_log(LogLevel::Info, "Fetching BTC/USDT (Binance)…".to_string());
            if let Ok(resp) = client.get(BINANCE_TICKER).send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    let price = json.get("price").and_then(|p| p.as_str()).unwrap_or("—");
                    if let Ok(mut c) = self.crypto.write() {
                        c.btc_usdt = format!("BTC/USDT {}", price);
                    }
                    self.push_crypto_log(LogLevel::Success, format!("BTC/USDT {}", price));
                } else {
                    self.push_crypto_log(
                        LogLevel::Warning,
                        "Binance ticker parse failed".to_string(),
                    );
                }
            } else {
                self.push_crypto_log(LogLevel::Error, "Binance request failed".to_string());
            }
            self.push_crypto_log(LogLevel::Info, "Fetching Gamma events…".to_string());
            if let Ok(resp) = client.get(GAMMA_EVENTS).send() {
                if let Ok(arr) = resp.json::<Vec<serde_json::Value>>() {
                    let lines: Vec<String> = arr
                        .iter()
                        .take(10)
                        .filter_map(|e| {
                            let title = e.get("title").and_then(|t| t.as_str())?;
                            let slug = e.get("slug").and_then(|s| s.as_str()).unwrap_or("");
                            Some(format!("{} | {}", title, slug))
                        })
                        .collect();
                    if let Ok(mut c) = self.crypto.write() {
                        c.events = lines.clone();
                    }
                    self.push_crypto_log(
                        LogLevel::Success,
                        format!("Loaded {} Gamma events", lines.len()),
                    );
                } else {
                    self.push_crypto_log(
                        LogLevel::Warning,
                        "Gamma events parse failed".to_string(),
                    );
                }
            } else {
                self.push_crypto_log(LogLevel::Error, "Gamma request failed".to_string());
            }
        } else {
            self.push_crypto_log(LogLevel::Error, "HTTP client init failed".to_string());
        }

        // ── Sports: fixtures + xG + strategy scan ───────────────────────────
        self.fetch_sports();

        // ── Weather ──────────────────────────────────────────────────────────
        if let Ok(meteo) = crate::weather::data::OpenMeteoClient::new() {
            self.push_weather_log(
                LogLevel::Info,
                "Fetching 7-day forecast (Open-Meteo NYC)…".to_string(),
            );
            match meteo.fetch_daily(40.7, -74.0) {
                Ok(daily) => {
                    let lines: Vec<String> = daily
                        .iter()
                        .take(7)
                        .map(|d| {
                            let max = d
                                .temperature_2m_max
                                .map(|t| t.to_string())
                                .unwrap_or_else(|| "—".to_string());
                            let min = d
                                .temperature_2m_min
                                .map(|t| t.to_string())
                                .unwrap_or_else(|| "—".to_string());
                            format!("{}  max {}°C  min {}°C", d.date, max, min)
                        })
                        .collect();
                    if let Ok(mut w) = self.weather.write() {
                        w.forecast = lines.clone();
                    }
                    self.push_weather_log(
                        LogLevel::Success,
                        format!("Loaded {} days", lines.len()),
                    );
                }
                Err(e) => {
                    self.push_weather_log(
                        LogLevel::Error,
                        format!("Open-Meteo fetch failed: {}", e),
                    );
                }
            }
        } else {
            self.push_weather_log(LogLevel::Error, "Client init failed".to_string());
        }
    }

    fn fetch_sports(&self) {
        // 1. Fetch fixtures.
        self.push_sports_log(LogLevel::Info, "Fetching fixtures…".to_string());
        let raw_fixtures = self.fetch_raw_fixtures();

        // 2. Fetch xG data (use cache if unavailable).
        let xg_map = self.fetch_xg_data();

        // 3. Fetch live match scores (for 70-min rule).
        let live_snapshots = self.fetch_live_scores();

        // 4. Build FixtureWithStats (attach xG + market discovery).
        let fixtures_with_stats = self.enrich_fixtures(raw_fixtures, &xg_map);

        // 5. Run enabled strategies.
        let new_signals = self.run_strategies(&fixtures_with_stats);

        // 6. Write results into SportsState.
        if let Ok(mut s) = self.sports.write() {
            s.fixtures = fixtures_with_stats;
            s.live_matches = live_snapshots;
            s.xg_cache = xg_map;
            // Append new signals (don't replace existing ones).
            s.signals.extend(new_signals);
            // Cap total signals at 200.
            let drain_count = s.signals.len().saturating_sub(200);
            if drain_count > 0 {
                s.signals.drain(0..drain_count);
            }
        }

        self.push_sports_log(LogLevel::Success, "Sports data updated".to_string());
    }

    fn fetch_raw_fixtures(&self) -> Vec<crate::sports::data::Fixture> {
        if let Ok(scraper) = crate::sports::data::SportsScraper::new() {
            match scraper.fetch_pl_fixtures() {
                Ok(f) => {
                    self.push_sports_log(
                        LogLevel::Info,
                        format!("FBRef/FixtureDownload: {} PL fixtures", f.len()),
                    );
                    return f;
                }
                Err(e) => {
                    self.push_sports_log(LogLevel::Warning, format!("Fixture fetch: {}", e));
                }
            }
        }
        Vec::new()
    }

    fn fetch_xg_data(&self) -> HashMap<String, crate::sports::data::TeamXgStats> {
        // Use cached data if available and non-empty.
        let cached_len = self.sports.read().map(|s| s.xg_cache.len()).unwrap_or(0);
        if cached_len > 0 {
            return self
                .sports
                .read()
                .map(|s| s.xg_cache.clone())
                .unwrap_or_default();
        }
        if let Ok(scraper) = crate::sports::data::XgScraper::new() {
            let map = scraper.fetch_pl_xg();
            if !map.is_empty() {
                self.push_sports_log(LogLevel::Info, format!("FBRef xG: {} teams", map.len()));
            }
            map
        } else {
            HashMap::new()
        }
    }

    fn fetch_live_scores(&self) -> Vec<LiveMatchSnapshot> {
        // Only fetch during typical match windows (12:00–23:00 UTC).
        let hour = chrono::Utc::now()
            .format("%H")
            .to_string()
            .parse::<u8>()
            .unwrap_or(0);
        if !(12..=23).contains(&hour) {
            return Vec::new();
        }
        if let Ok(client) = crate::sports::data::LiveScoresClient::new() {
            client
                .fetch_live()
                .into_iter()
                .map(|lm| LiveMatchSnapshot {
                    home_team: lm.home_team,
                    away_team: lm.away_team,
                    home_goals: lm.home_goals,
                    away_goals: lm.away_goals,
                    minute: lm.minute,
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn enrich_fixtures(
        &self,
        raw: Vec<crate::sports::data::Fixture>,
        xg_map: &HashMap<String, crate::sports::data::TeamXgStats>,
    ) -> Vec<crate::sports::FixtureWithStats> {
        let discovery = crate::sports::discovery::MarketDiscovery::new().ok();

        raw.into_iter()
            .map(|f| {
                let mut fws = crate::sports::discovery::FixtureWithStats::from_fixture(f.clone());

                // Attach xG data if available.
                if let Some(home_stats) = xg_map.get(&f.home) {
                    fws.home_xg_per_90 = home_stats.xg_per_90.max(0.1);
                    fws.home_xga_per_90 = home_stats.xga_per_90.max(0.1);
                    fws.home_win_rate = home_stats.home_win_rate;
                }
                if let Some(away_stats) = xg_map.get(&f.away) {
                    fws.away_xg_per_90 = away_stats.xg_per_90.max(0.1);
                    fws.away_xga_per_90 = away_stats.xga_per_90.max(0.1);
                    fws.away_win_rate = away_stats.away_win_rate;
                }

                // Try to find a matching Polymarket market.
                if let Some(ref disc) = discovery {
                    fws.polymarket = disc.find_market(&f);
                }

                fws
            })
            .collect()
    }

    fn run_strategies(&self, fixtures: &[crate::sports::FixtureWithStats]) -> Vec<StoredSignal> {
        let configs = self
            .sports
            .read()
            .map(|s| s.strategy_configs.clone())
            .unwrap_or_default();

        let registry = crate::sports::strategies::StrategyRegistry::default();
        let raw_signals = registry.scan(fixtures);

        // Convert SportsSignal → StoredSignal, applying enabled config.
        let enabled_ids: Vec<&str> = configs.iter().filter(|c| c.enabled).map(|c| c.id).collect();

        raw_signals
            .into_iter()
            .filter(|s| enabled_ids.contains(&s.signal.strategy_id.as_str()))
            .map(|s| StoredSignal {
                market_id: s.signal.market_id.clone(),
                home: s.fixture.fixture.home.clone(),
                away: s.fixture.fixture.away.clone(),
                date: s.fixture.fixture.date.clone(),
                side: match s.signal.side {
                    crate::shared::strategy::Side::Yes => "YES".to_string(),
                    crate::shared::strategy::Side::No => "NO".to_string(),
                },
                edge_pct: s.signal.edge_pct,
                kelly_size: s.signal.kelly_size,
                strategy_id: s.signal.strategy_id.clone(),
                status: "pending".to_string(),
                created_at: s.created_at,
            })
            .collect()
    }
}
