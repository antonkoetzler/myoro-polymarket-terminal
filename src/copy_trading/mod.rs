//! Copy-trading: monitor trades from profiles listed in config.json (copy_traders).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use crate::config;
use crate::shared::execution::Executor;
use crate::shared::strategy::Side;

const DATA_API: &str = "https://data-api.polymarket.com";
const MAX_DISPLAY: usize = 24;
const MIN_COPY_USD: f64 = 0.01;

fn is_valid_address(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("0x") && s.len() == 42 && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn normalize_address(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("0x") {
        s.to_string()
    } else {
        format!("0x{}", s)
    }
}

/// Copy trader list backed by config.json.
pub struct TraderList {
    config: Arc<RwLock<config::Config>>,
}

impl TraderList {
    pub fn new(config: Arc<RwLock<config::Config>>) -> Self {
        Self { config }
    }

    pub fn reload_if_changed(&self) {
        // Config is in memory; reload from file if we want external edits. For now no-op.
    }

    pub fn get_addresses(&self) -> Vec<String> {
        self.config
            .read()
            .map(|c| c.copy_traders.clone())
            .unwrap_or_default()
    }

    pub fn add(&self, addr: String) -> bool {
        let n = normalize_address(&addr);
        if !is_valid_address(&n) {
            return false;
        }
        if let Ok(mut c) = self.config.write() {
            if c.copy_traders.contains(&n) {
                return true;
            }
            c.copy_traders.push(n);
            let _ = config::save_config(&c);
            true
        } else {
            false
        }
    }

    pub fn remove_at(&self, index: usize) {
        if let Ok(mut c) = self.config.write() {
            if index < c.copy_traders.len() {
                c.copy_traders.remove(index);
                let _ = config::save_config(&c);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.config
            .read()
            .map(|c| c.copy_traders.len())
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub struct TradeRow {
    pub user: String,
    pub side: String,
    pub size: f64,
    pub price: f64,
    pub title: String,
    pub outcome: String,
    pub ts: i64,
    pub tx: String,
    pub condition_id: Option<String>,
    pub asset_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiTrade {
    #[serde(rename = "proxyWallet")]
    proxy_wallet: Option<String>,
    side: Option<String>,
    size: Option<f64>,
    price: Option<f64>,
    title: Option<String>,
    outcome: Option<String>,
    #[serde(rename = "conditionId")]
    condition_id: Option<String>,
    asset: Option<String>,
    timestamp: Option<i64>,
    #[serde(rename = "transactionHash")]
    transaction_hash: Option<String>,
}

#[derive(Debug, Serialize)]
struct PaperTradeRecord<'a> {
    timestamp: String,
    source_timestamp: i64,
    condition_id: &'a str,
    asset_id: Option<&'a str>,
    side: &'a str,
    size: f64,
    price: f64,
    title: &'a str,
    outcome: &'a str,
    source_trader_address: &'a str,
    source_transaction_hash: &'a str,
}

fn parse_copy_side(side: &str) -> Option<Side> {
    if side.eq_ignore_ascii_case("buy") || side.eq_ignore_ascii_case("yes") {
        Some(Side::Yes)
    } else if side.eq_ignore_ascii_case("sell") || side.eq_ignore_ascii_case("no") {
        Some(Side::No)
    } else {
        None
    }
}

fn lookup_trader_bankroll(config: &config::Config, trader_addr: &str) -> Option<f64> {
    if let Some(v) = config.copy_trader_bankrolls.get(trader_addr).copied() {
        return Some(v);
    }
    let needle = trader_addr.to_lowercase();
    config
        .copy_trader_bankrolls
        .iter()
        .find_map(|(k, v)| (k.to_lowercase() == needle).then_some(*v))
}

pub(crate) fn compute_copy_size(
    config: &config::Config,
    trader_addr: &str,
    trader_size: f64,
) -> Option<f64> {
    if trader_size <= 0.0 {
        return None;
    }
    let my_bankroll = config.paper_bankroll?;
    if my_bankroll <= 0.0 {
        return None;
    }
    let raw = match config.copy_sizing {
        config::CopySizing::Proportional => {
            let trader_bankroll = lookup_trader_bankroll(config, trader_addr)?;
            if trader_bankroll <= 0.0 {
                return None;
            }
            (my_bankroll / trader_bankroll) * trader_size
        }
        config::CopySizing::Fixed => my_bankroll * config.copy_bankroll_fraction,
    };
    let sized = raw.min(config.copy_max_usd.max(MIN_COPY_USD));
    (sized >= MIN_COPY_USD).then_some(sized)
}

fn append_paper_trade_jsonl(path: &str, trade: &TradeRow, size: f64) -> anyhow::Result<()> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let record = PaperTradeRecord {
        timestamp: chrono::Utc::now().to_rfc3339(),
        source_timestamp: trade.ts,
        condition_id: trade.condition_id.as_deref().unwrap_or_default(),
        asset_id: trade.asset_id.as_deref(),
        side: &trade.side,
        size,
        price: trade.price,
        title: &trade.title,
        outcome: &trade.outcome,
        source_trader_address: &trade.user,
        source_transaction_hash: &trade.tx,
    };
    let json = serde_json::to_string(&record)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", json)?;
    Ok(())
}

/// Runs the execution loop for fetched trades: execute, log, and append paper trades.
/// Used by Monitor::poll_once and by tests.
pub(crate) fn execute_copy_trades(
    trades: &[TradeRow],
    cfg: &config::Config,
    log_sink: Option<&crate::live::LiveState>,
) {
    if !cfg.copy_auto_execute {
        return;
    }
    let exec = Executor::new(cfg.execution_mode);
    for r in trades {
        if r.condition_id.is_none() || r.asset_id.is_none() {
            continue;
        }
        let Some(side) = parse_copy_side(&r.side) else {
            continue;
        };
        let Some(amount) = compute_copy_size(cfg, &r.user, r.size) else {
            if let Some(live) = log_sink {
                live.push_copy_log(
                    crate::live::LogLevel::Warning,
                    format!("Skipped copy trade for {} (invalid sizing inputs)", r.user),
                );
            }
            continue;
        };
        if exec
            .execute(r.condition_id.as_deref().unwrap_or_default(), side, amount)
            .is_err()
        {
            if let Some(live) = log_sink {
                live.push_copy_log(
                    crate::live::LogLevel::Error,
                    format!("Copy execute failed for {}", r.title),
                );
            }
            continue;
        }
        if cfg.execution_mode == config::ExecutionMode::Paper {
            if let Some(live) = log_sink {
                live.push_copy_log(
                    crate::live::LogLevel::Success,
                    format!(
                        "Paper copy: {} {:.4} @ {} · {}",
                        r.side, amount, r.price, r.title
                    ),
                );
            }
            let _ = append_paper_trade_jsonl(&cfg.paper_trades_file, r, amount);
        }
    }
}

pub struct Monitor {
    list: std::sync::Arc<TraderList>,
    trades: RwLock<Vec<TradeRow>>,
    seen: RwLock<HashSet<String>>,
    log_sink: Option<std::sync::Arc<crate::live::LiveState>>,
    running: std::sync::Arc<AtomicBool>,
}

impl Monitor {
    pub fn poll_ms_from_config(config: &config::Config) -> u64 {
        config.copy_poll_ms
    }

    pub fn new(
        list: std::sync::Arc<TraderList>,
        log_sink: Option<std::sync::Arc<crate::live::LiveState>>,
        running: std::sync::Arc<AtomicBool>,
    ) -> Self {
        Self {
            list,
            trades: RwLock::new(Vec::new()),
            seen: RwLock::new(HashSet::new()),
            log_sink,
            running,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn set_running(&self, v: bool) {
        self.running.store(v, Ordering::SeqCst);
    }

    pub fn trader_list(&self) -> &std::sync::Arc<TraderList> {
        &self.list
    }

    pub fn poll_once(&self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }
        let cfg = match self.list.config.read() {
            Ok(c) => c.clone(),
            Err(_) => return,
        };
        self.list.reload_if_changed();
        let addresses = self.list.get_addresses();
        if addresses.is_empty() {
            return;
        }
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut all: Vec<TradeRow> = Vec::new();
        for addr in &addresses {
            let url = format!("{}/trades?user={}&limit=30&takerOnly=false", DATA_API, addr);
            let resp = match client.get(&url).send() {
                Ok(r) => r,
                Err(_) => continue,
            };
            let list: Vec<ApiTrade> = match resp.json() {
                Ok(l) => l,
                Err(_) => continue,
            };
            for t in list {
                let tx = t.transaction_hash.unwrap_or_default();
                if tx.is_empty() {
                    continue;
                }
                let key = format!("{}:{}", tx, t.timestamp.unwrap_or(0));
                {
                    let mut seen = match self.seen.write() {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    if seen.contains(&key) {
                        continue;
                    }
                    seen.insert(key);
                }
                all.push(TradeRow {
                    user: t.proxy_wallet.as_deref().unwrap_or("?").to_string(),
                    side: t.side.unwrap_or_else(|| "?".to_string()),
                    size: t.size.unwrap_or(0.0),
                    price: t.price.unwrap_or(0.0),
                    title: t.title.unwrap_or_else(|| "—".to_string()),
                    outcome: t.outcome.unwrap_or_else(|| "—".to_string()),
                    ts: t.timestamp.unwrap_or(0),
                    tx: tx.clone(),
                    condition_id: t.condition_id.clone(),
                    asset_id: t.asset.clone(),
                });
            }
        }
        if all.is_empty() {
            return;
        }
        all.sort_by(|a, b| b.ts.cmp(&a.ts));
        if let Some(ref live) = self.log_sink {
            for r in &all {
                live.push_log(format!(
                    "Copy trade: {} {} @ {} · {}",
                    r.side, r.size, r.price, r.title
                ));
            }
        }
        execute_copy_trades(&all, &cfg, self.log_sink.as_ref().map(Arc::as_ref));
        let mut trades = match self.trades.write() {
            Ok(t) => t,
            Err(_) => return,
        };
        for r in all {
            trades.insert(0, r);
        }
        trades.truncate(200);
    }

    pub fn recent_trades(&self, n: usize) -> Vec<TradeRow> {
        self.trades
            .read()
            .map(|t| t.iter().take(n).cloned().collect())
            .unwrap_or_default()
    }

    pub fn copy_tab_display(&self, selected_index: Option<usize>, _input_buf: &str) -> String {
        let addresses = self.list.get_addresses();
        let mut out = String::new();
        for (i, addr) in addresses.iter().enumerate() {
            let mark = if Some(i) == selected_index {
                "► "
            } else {
                "  "
            };
            let short = addr
                .get(..10)
                .map(|s| format!("{}…", s))
                .unwrap_or_else(|| addr.clone());
            out.push_str(&format!("{}{}\n", mark, short));
        }
        if addresses.is_empty() {
            out.push_str("No profiles. Add from Discover (a/Enter) or Shortcuts screen.");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> config::Config {
        config::Config {
            execution_mode: config::ExecutionMode::Paper,
            polymarket: config::PolymarketConfig::default(),
            binance: config::BinanceConfig::default(),
            paper_bankroll: Some(10.0),
            copy_traders: vec![],
            copy_poll_ms: 250,
            pnl_currency: "USD".to_string(),
            copy_sizing: config::CopySizing::Proportional,
            copy_trader_bankrolls: std::collections::HashMap::new(),
            copy_bankroll_fraction: 0.05,
            copy_max_usd: 1000.0,
            copy_auto_execute: false,
            paper_trades_file: "data/paper_copy_trades.jsonl".to_string(),
        }
    }

    #[test]
    fn trader_list_validates_and_removes() {
        let cfg = Arc::new(RwLock::new(base_config()));
        let list = TraderList::new(Arc::clone(&cfg));
        assert!(!list.add("bad".to_string()));
        assert!(list.add("0x1234567890123456789012345678901234567890".to_string()));
        assert_eq!(list.len(), 1);
        list.remove_at(0);
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn copy_size_proportional_and_fixed() {
        let mut cfg = base_config();
        cfg.copy_trader_bankrolls
            .insert("0xabc".to_string(), 10_000.0);
        let proportional = compute_copy_size(&cfg, "0xabc", 1000.0).expect("size");
        assert!((proportional - 1.0).abs() < 1e-9);

        cfg.copy_sizing = config::CopySizing::Fixed;
        cfg.copy_bankroll_fraction = 0.2;
        cfg.copy_max_usd = 1.5;
        let fixed = compute_copy_size(&cfg, "0xabc", 1000.0).expect("fixed");
        assert!((fixed - 1.5).abs() < 1e-9);
    }

    #[test]
    fn recent_trades_returns_newest_first() {
        let cfg = Arc::new(RwLock::new(base_config()));
        let list = Arc::new(TraderList::new(cfg));
        let monitor = Monitor::new(list, None, Arc::new(AtomicBool::new(true)));
        if let Ok(mut trades) = monitor.trades.write() {
            trades.push(TradeRow {
                user: "u1".to_string(),
                side: "BUY".to_string(),
                size: 1.0,
                price: 0.5,
                title: "A".to_string(),
                outcome: "YES".to_string(),
                ts: 1,
                tx: "t1".to_string(),
                condition_id: Some("c1".to_string()),
                asset_id: Some("a1".to_string()),
            });
            trades.push(TradeRow {
                user: "u2".to_string(),
                side: "SELL".to_string(),
                size: 2.0,
                price: 0.4,
                title: "B".to_string(),
                outcome: "NO".to_string(),
                ts: 2,
                tx: "t2".to_string(),
                condition_id: Some("c2".to_string()),
                asset_id: Some("a2".to_string()),
            });
        }
        let rows = monitor.recent_trades(1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tx, "t1");
    }

    #[test]
    fn append_paper_trade_writes_jsonl() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paper_copy_test_{}", ts));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let file = dir.join("paper_copy_trades.jsonl");
        let trade = TradeRow {
            user: "0xabc".to_string(),
            side: "BUY".to_string(),
            size: 2.0,
            price: 0.6,
            title: "Market".to_string(),
            outcome: "YES".to_string(),
            ts: 10,
            tx: "0xtx".to_string(),
            condition_id: Some("0xcond".to_string()),
            asset_id: Some("123".to_string()),
        };
        append_paper_trade_jsonl(file.to_str().expect("path"), &trade, 1.25).expect("append");
        let body = std::fs::read_to_string(&file).expect("read");
        assert!(body.contains("\"condition_id\":\"0xcond\""));
        assert!(body.contains("\"size\":1.25"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_copy_trades_with_auto_execute_writes_paper_and_logs() {
        let mut cfg = base_config();
        cfg.copy_auto_execute = true;
        cfg.copy_trader_bankrolls
            .insert("0xuser".to_string(), 1000.0);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("execute_copy_test_{}", ts));
        std::fs::create_dir_all(&dir).expect("mkdir");
        cfg.paper_trades_file = dir.join("trades.jsonl").to_str().expect("path").to_string();

        let live = Arc::new(crate::live::LiveState::default());
        let trade = TradeRow {
            user: "0xuser".to_string(),
            side: "BUY".to_string(),
            size: 10.0,
            price: 0.55,
            title: "Test Market".to_string(),
            outcome: "YES".to_string(),
            ts: 1,
            tx: "0xtx".to_string(),
            condition_id: Some("cond1".to_string()),
            asset_id: Some("asset1".to_string()),
        };

        execute_copy_trades(&[trade], &cfg, Some(live.as_ref()));

        let body = std::fs::read_to_string(&cfg.paper_trades_file).expect("read");
        assert!(body.contains("cond1"));
        assert!(body.contains("Test Market"));

        let logs = live.get_copy_logs();
        let has_paper_copy = logs.iter().any(|(_, msg)| msg.contains("Paper copy:"));
        assert!(
            has_paper_copy,
            "expected log to contain 'Paper copy:', got {:?}",
            logs
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_copy_trades_without_auto_execute_does_not_write() {
        let mut cfg = base_config();
        cfg.copy_auto_execute = false;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("execute_copy_no_exec_{}", ts));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let file_path = dir.join("trades.jsonl");
        cfg.paper_trades_file = file_path.to_str().expect("path").to_string();

        let trade = TradeRow {
            user: "0xuser".to_string(),
            side: "BUY".to_string(),
            size: 10.0,
            price: 0.5,
            title: "M".to_string(),
            outcome: "YES".to_string(),
            ts: 1,
            tx: "tx".to_string(),
            condition_id: Some("c".to_string()),
            asset_id: Some("a".to_string()),
        };

        execute_copy_trades(&[trade], &cfg, None);

        assert!(
            !file_path.exists(),
            "file should not be created when copy_auto_execute is false"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
