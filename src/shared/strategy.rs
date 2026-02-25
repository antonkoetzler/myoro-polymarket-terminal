//! Strategy trait and registry; domains implement this.

use anyhow::Result;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Signal {
    pub market_id: String,
    pub side: Side,
    pub confidence: f64,
    /// Edge percentage e.g. 0.12 = 12% edge over market price.
    pub edge_pct: f64,
    /// Fractional Kelly stake (0.0–1.0); multiply by bankroll for dollar amount.
    pub kelly_size: f64,
    /// Auto-execute immediately if true; otherwise queue for manual confirmation.
    pub auto_execute: bool,
    /// Strategy that generated this signal.
    pub strategy_id: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub enum Side {
    Yes,
    No,
}

pub trait Strategy: Send + Sync {
    fn id(&self) -> &'static str;
    fn metadata(&self) -> StrategyMetadata;
    fn signal(&self) -> Result<Option<Signal>>;
}

#[derive(Clone, Debug)]
pub struct StrategyMetadata {
    pub name: &'static str,
    pub domain: &'static str,
}
