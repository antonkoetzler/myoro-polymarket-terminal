#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const CONFIG_JSON: &str = "config.json";

/// Fields persisted in config.json (no credentials).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JsonConfigFile {
    pub paper_bankroll: Option<f64>,
    pub execution_mode: Option<String>,
    #[serde(default)]
    pub copy_traders: Vec<String>,
    #[serde(default)]
    pub copy_poll_ms: Option<u64>,
    pub pnl_currency: Option<String>,
    pub copy_sizing: Option<CopySizing>,
    #[serde(default)]
    pub copy_trader_bankrolls: HashMap<String, f64>,
    pub copy_bankroll_fraction: Option<f64>,
    pub copy_max_usd: Option<f64>,
    pub copy_auto_execute: Option<bool>,
    pub paper_trades_file: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    pub execution_mode: ExecutionMode,
    #[serde(default)]
    pub polymarket: PolymarketConfig,
    #[serde(default)]
    pub binance: BinanceConfig,
    #[serde(default)]
    pub paper_bankroll: Option<f64>,
    #[serde(default)]
    pub copy_traders: Vec<String>,
    #[serde(default)]
    pub copy_poll_ms: u64,
    #[serde(default)]
    pub pnl_currency: String,
    #[serde(default)]
    pub copy_sizing: CopySizing,
    #[serde(default)]
    pub copy_trader_bankrolls: HashMap<String, f64>,
    #[serde(default)]
    pub copy_bankroll_fraction: f64,
    #[serde(default)]
    pub copy_max_usd: f64,
    #[serde(default)]
    pub copy_auto_execute: bool,
    #[serde(default)]
    pub paper_trades_file: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CopySizing {
    #[default]
    Proportional,
    Fixed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    #[default]
    Paper,
    Live,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PolymarketConfig {
    /// Proxy (Safe) address so CLOB orders show under your Polymarket profile.
    pub funder_address: Option<String>,
    pub private_key: Option<String>,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub api_passphrase: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BinanceConfig {
    pub api_key: Option<String>,
}

/// Parse EXECUTION_MODE string. Used by load() and by tests.
pub fn parse_execution_mode(s: Option<&str>) -> ExecutionMode {
    match s {
        Some(s) if s.eq_ignore_ascii_case("live") => ExecutionMode::Live,
        _ => ExecutionMode::Paper,
    }
}

pub fn load() -> Result<Config> {
    let _ = dotenvy::dotenv();
    let mut config = Config {
        execution_mode: ExecutionMode::Paper,
        polymarket: PolymarketConfig {
            funder_address: std::env::var("FUNDER_ADDRESS").ok(),
            private_key: std::env::var("PRIVATE_KEY").ok(),
            api_key: std::env::var("API_KEY").ok(),
            api_secret: std::env::var("API_SECRET").ok(),
            api_passphrase: std::env::var("API_PASSPHRASE").ok(),
        },
        binance: BinanceConfig {
            api_key: std::env::var("BINANCE_API_KEY").ok(),
        },
        paper_bankroll: None,
        copy_traders: Vec::new(),
        copy_poll_ms: 250,
        pnl_currency: "USD".to_string(),
        copy_sizing: CopySizing::Proportional,
        copy_trader_bankrolls: HashMap::new(),
        copy_bankroll_fraction: 0.05,
        copy_max_usd: 1000.0,
        copy_auto_execute: false,
        paper_trades_file: "data/paper_copy_trades.jsonl".to_string(),
    };
    if let Ok(data) = std::fs::read_to_string(CONFIG_JSON) {
        if let Ok(file) = serde_json::from_str::<JsonConfigFile>(&data) {
            config.paper_bankroll = file.paper_bankroll.or(config.paper_bankroll);
            config.execution_mode = file
                .execution_mode
                .as_deref()
                .map(|s| parse_execution_mode(Some(s)))
                .unwrap_or(config.execution_mode);
            config.copy_traders = file.copy_traders;
            config.copy_poll_ms = file.copy_poll_ms.unwrap_or(250).clamp(100, 30_000);
            config.copy_sizing = file.copy_sizing.unwrap_or(CopySizing::Proportional);
            config.copy_trader_bankrolls = file.copy_trader_bankrolls;
            let fraction = file.copy_bankroll_fraction.unwrap_or(0.05);
            config.copy_bankroll_fraction = if (0.0..=1.0).contains(&fraction) && fraction > 0.0 {
                fraction
            } else {
                0.05
            };
            config.copy_max_usd = file.copy_max_usd.unwrap_or(1000.0).max(0.01);
            config.copy_auto_execute = file.copy_auto_execute.unwrap_or(false);
            if let Some(path) = file.paper_trades_file {
                if !path.trim().is_empty() {
                    config.paper_trades_file = path;
                }
            }
            if let Some(c) = file.pnl_currency {
                if !c.is_empty() {
                    config.pnl_currency = c;
                }
            }
        }
    }
    Ok(config)
}

/// Save dynamic settings to config.json (no credentials).
pub fn save_config(c: &Config) -> Result<()> {
    let file = JsonConfigFile {
        paper_bankroll: c.paper_bankroll,
        execution_mode: Some(
            match c.execution_mode {
                ExecutionMode::Paper => "paper",
                ExecutionMode::Live => "live",
            }
            .to_string(),
        ),
        copy_traders: c.copy_traders.clone(),
        copy_poll_ms: Some(c.copy_poll_ms),
        pnl_currency: Some(c.pnl_currency.clone()),
        copy_sizing: Some(c.copy_sizing),
        copy_trader_bankrolls: c.copy_trader_bankrolls.clone(),
        copy_bankroll_fraction: Some(c.copy_bankroll_fraction),
        copy_max_usd: Some(c.copy_max_usd),
        copy_auto_execute: Some(c.copy_auto_execute),
        paper_trades_file: Some(c.paper_trades_file.clone()),
    };
    let s = serde_json::to_string_pretty(&file)?;
    std::fs::write(CONFIG_JSON, s)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_mode_default_is_paper() {
        assert_eq!(parse_execution_mode(None), ExecutionMode::Paper);
    }

    #[test]
    fn execution_mode_parse_live() {
        assert_eq!(parse_execution_mode(Some("live")), ExecutionMode::Live);
        assert_eq!(parse_execution_mode(Some("LIVE")), ExecutionMode::Live);
    }

    #[test]
    fn execution_mode_parse_non_live_is_paper() {
        assert_eq!(parse_execution_mode(Some("paper")), ExecutionMode::Paper);
        assert_eq!(parse_execution_mode(Some("")), ExecutionMode::Paper);
        assert_eq!(parse_execution_mode(Some("other")), ExecutionMode::Paper);
    }

    #[test]
    fn json_config_roundtrip_copy_fields() {
        let mut file = JsonConfigFile::default();
        file.copy_sizing = Some(CopySizing::Fixed);
        file.copy_trader_bankrolls
            .insert("0xabc".to_string(), 1000.0);
        file.copy_bankroll_fraction = Some(0.25);
        file.copy_max_usd = Some(22.5);
        file.copy_auto_execute = Some(true);
        file.paper_trades_file = Some("data/custom.jsonl".to_string());
        let s = serde_json::to_string(&file).expect("serialize");
        let parsed: JsonConfigFile = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(parsed.copy_sizing, Some(CopySizing::Fixed));
        assert_eq!(parsed.copy_bankroll_fraction, Some(0.25));
        assert_eq!(parsed.copy_max_usd, Some(22.5));
        assert_eq!(parsed.copy_auto_execute, Some(true));
        assert_eq!(
            parsed.paper_trades_file.as_deref(),
            Some("data/custom.jsonl")
        );
        assert_eq!(
            parsed.copy_trader_bankrolls.get("0xabc").copied(),
            Some(1000.0)
        );
    }

    #[test]
    fn config_struct_defaults_copy_fields() {
        let cfg = Config::default();
        assert_eq!(cfg.copy_sizing, CopySizing::Proportional);
        assert_eq!(cfg.copy_bankroll_fraction, 0.0);
        assert_eq!(cfg.copy_max_usd, 0.0);
        assert!(!cfg.copy_auto_execute);
        assert_eq!(cfg.paper_trades_file, "");
        assert!(cfg.copy_trader_bankrolls.is_empty());
    }
}
