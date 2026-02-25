//! Cross-platform arbitrage scanner: Polymarket vs Kalshi price discrepancy detection.

use crate::shared::strategy::{Side, Signal};
use crate::sports::data::kalshi::KalshiMarket;
use crate::sports::discovery::FixtureWithStats;
use crate::sports::signals::SportsSignal;
use crate::sports::strategies::SportsStrategy;

const STRATEGY_ID: &str = "arb_scanner";
const STRATEGY_NAME: &str = "Cross-Platform Arb";
const STRATEGY_DESC: &str =
    "Detects pure arb: Polymarket YES + Kalshi NO < 1.0. Flags opportunity (Kalshi exec TBD).";

/// Minimum guaranteed profit (after fees) to emit a signal.
const FEE_THRESHOLD: f64 = 0.005;

pub struct ArbScannerStrategy {
    enabled: bool,
    auto_execute: bool,
    kalshi_markets: std::sync::RwLock<Vec<KalshiMarket>>,
}

impl ArbScannerStrategy {
    pub fn new() -> Self {
        Self {
            enabled: false,
            auto_execute: false,
            kalshi_markets: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Update the Kalshi market cache (called by background thread).
    pub fn update_kalshi_markets(&self, markets: Vec<KalshiMarket>) {
        if let Ok(mut g) = self.kalshi_markets.write() {
            *g = markets;
        }
    }

    fn fuzzy_match<'a>(
        &self,
        fixture: &FixtureWithStats,
        kalshi: &'a [KalshiMarket],
    ) -> Option<&'a KalshiMarket> {
        let home_lc = fixture.fixture.home.to_lowercase();
        let away_lc = fixture.fixture.away.to_lowercase();

        kalshi.iter().find(|km| {
            let title = km.title.to_lowercase();
            title.contains(&home_lc) || title.contains(&away_lc)
        })
    }
}

impl SportsStrategy for ArbScannerStrategy {
    fn id(&self) -> &str {
        STRATEGY_ID
    }
    fn name(&self) -> &str {
        STRATEGY_NAME
    }
    fn description(&self) -> &str {
        STRATEGY_DESC
    }
    fn is_custom(&self) -> bool {
        false
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
    }
    fn auto_execute(&self) -> bool {
        self.auto_execute
    }

    fn scan(&self, fixtures: &[FixtureWithStats]) -> Vec<SportsSignal> {
        let kalshi = match self.kalshi_markets.read() {
            Ok(g) => g.clone(),
            Err(_) => return Vec::new(),
        };
        if kalshi.is_empty() {
            return Vec::new();
        }
        fixtures
            .iter()
            .filter(|f| f.fixture.home_goals.is_none())
            .filter_map(|f| {
                let km = self.fuzzy_match(f, &kalshi)?;
                self.evaluate(f, km)
            })
            .collect()
    }
}

impl ArbScannerStrategy {
    fn evaluate(&self, f: &FixtureWithStats, km: &KalshiMarket) -> Option<SportsSignal> {
        let pm = f.polymarket.as_ref()?;

        // Pure arb: buy Polymarket YES at pm.yes_price and Kalshi NO at km.no_price.
        let combined = pm.yes_price + km.no_price;
        let edge = 1.0 - combined;

        if edge <= FEE_THRESHOLD {
            return None;
        }

        Some(SportsSignal::new(
            Signal {
                market_id: pm.asset_id.clone(),
                side: Side::Yes,
                confidence: 1.0, // guaranteed arb
                edge_pct: edge,
                kelly_size: edge, // in a true arb, bet full edge fraction
                auto_execute: self.auto_execute,
                strategy_id: STRATEGY_ID.to_string(),
                metadata: Some(serde_json::json!({
                    "home": f.fixture.home,
                    "away": f.fixture.away,
                    "poly_yes_price": pm.yes_price,
                    "kalshi_no_price": km.no_price,
                    "kalshi_ticker": km.ticker,
                    "edge": edge,
                    "note": "Buy YES on Polymarket, NO on Kalshi — guaranteed profit",
                })),
            },
            f.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sports::data::{kalshi::KalshiMarket, Fixture};
    use crate::sports::discovery::PolymarketMarket;

    fn fixture_with_pm(yes_price: f64) -> FixtureWithStats {
        let mut f = crate::sports::discovery::FixtureWithStats::from_fixture(Fixture {
            date: "2025-03-01".to_string(),
            home: "Arsenal".to_string(),
            away: "Chelsea".to_string(),
            home_goals: None,
            away_goals: None,
        });
        f.polymarket = Some(PolymarketMarket {
            condition_id: "c1".to_string(),
            asset_id: "a1".to_string(),
            yes_price,
            no_price: 1.0 - yes_price,
            title: "Arsenal vs Chelsea".to_string(),
        });
        f
    }

    fn kalshi_market(yes_price: f64) -> KalshiMarket {
        KalshiMarket {
            ticker: "SOCCER-ARS-WIN".to_string(),
            title: "Arsenal to win".to_string(),
            yes_price,
            no_price: 1.0 - yes_price,
        }
    }

    #[test]
    fn arb_detected_when_prices_below_1() {
        let strat = ArbScannerStrategy::new();
        // poly YES=0.47, kalshi NO=0.50 → combined=0.97, edge=0.03 > FEE_THRESHOLD
        let f = fixture_with_pm(0.47);
        let km = kalshi_market(0.50); // kalshi yes=0.50, no=0.50
        let sig = strat.evaluate(&f, &km);
        assert!(sig.is_some());
        let s = sig.unwrap();
        assert!(
            (s.signal.edge_pct - 0.03).abs() < 0.001,
            "edge={}",
            s.signal.edge_pct
        );
    }

    #[test]
    fn no_arb_when_no_discrepancy() {
        let strat = ArbScannerStrategy::new();
        // poly YES=0.55, kalshi NO=0.46 → combined=1.01 > 1.0 → no arb
        let f = fixture_with_pm(0.55);
        let km = kalshi_market(0.54); // kalshi no=0.46
        let sig = strat.evaluate(&f, &km);
        assert!(sig.is_none());
    }

    #[test]
    fn no_arb_below_fee_threshold() {
        let strat = ArbScannerStrategy::new();
        // poly YES=0.50, kalshi NO=0.497 → edge=0.003 < FEE_THRESHOLD=0.005
        let f = fixture_with_pm(0.50);
        let km = KalshiMarket {
            ticker: "T".to_string(),
            title: "Arsenal to win".to_string(),
            yes_price: 0.503,
            no_price: 0.497,
        };
        let sig = strat.evaluate(&f, &km);
        assert!(sig.is_none());
    }
}
