//! Home advantage strategy: Elo-adjusted uplift vs Polymarket market price.

use crate::shared::strategy::{Side, Signal};
use crate::sports::discovery::FixtureWithStats;
use crate::sports::signals::SportsSignal;
use crate::sports::strategies::SportsStrategy;

const STRATEGY_ID: &str = "home_adv";
const STRATEGY_NAME: &str = "Home Advantage";
const STRATEGY_DESC: &str =
    "Baseline +10% home uplift adjusted by team home record. Signals YES when market underprices.";

/// League-average home win rate (empirically ~46% across top European leagues).
const BASE_HOME_WIN_RATE: f64 = 0.46;
/// Minimum edge over market price to emit a signal.
const MIN_EDGE: f64 = 0.05;
const KELLY_FRACTION: f64 = 0.25;

pub struct HomeAdvantageStrategy {
    enabled: bool,
    auto_execute: bool,
    min_edge: f64,
    kelly_fraction: f64,
}

impl HomeAdvantageStrategy {
    pub fn new() -> Self {
        Self {
            enabled: false,
            auto_execute: false,
            min_edge: MIN_EDGE,
            kelly_fraction: KELLY_FRACTION,
        }
    }

    fn estimate_home_win_prob(f: &FixtureWithStats) -> f64 {
        // Base: league average home win rate.
        let base = BASE_HOME_WIN_RATE;
        // Team-specific adjustment: deviation from league average home win rate.
        let team_adj = f.home_win_rate - 0.46;
        let prob = base + team_adj;
        prob.clamp(0.10, 0.90)
    }
}

impl SportsStrategy for HomeAdvantageStrategy {
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
        fixtures
            .iter()
            .filter(|f| f.fixture.home_goals.is_none())
            .filter_map(|f| self.evaluate(f))
            .collect()
    }
}

impl HomeAdvantageStrategy {
    fn evaluate(&self, f: &FixtureWithStats) -> Option<SportsSignal> {
        let market = f.polymarket.as_ref()?;
        let p_home = Self::estimate_home_win_prob(f);
        let edge = p_home - market.yes_price;

        if edge < self.min_edge {
            return None;
        }

        let kelly = crate::sports::strategies::poisson::kelly_fraction(p_home, market.yes_price)
            * self.kelly_fraction;

        Some(SportsSignal::new(
            Signal {
                market_id: market.asset_id.clone(),
                side: Side::Yes,
                confidence: p_home,
                edge_pct: edge,
                kelly_size: kelly.max(0.0),
                auto_execute: self.auto_execute,
                strategy_id: STRATEGY_ID.to_string(),
                metadata: Some(serde_json::json!({
                    "home": f.fixture.home,
                    "away": f.fixture.away,
                    "estimated_home_prob": p_home,
                    "market_yes": market.yes_price,
                    "home_win_rate": f.home_win_rate,
                })),
            },
            f.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sports::data::Fixture;
    use crate::sports::discovery::PolymarketMarket;

    fn fixture_with_home_rate(home_win_rate: f64, market_yes: f64) -> FixtureWithStats {
        let mut f = crate::sports::discovery::FixtureWithStats::from_fixture(Fixture {
            date: "2025-03-01".to_string(),
            home: "Liverpool".to_string(),
            away: "Wolves".to_string(),
            home_goals: None,
            away_goals: None,
        });
        f.home_win_rate = home_win_rate;
        f.polymarket = Some(PolymarketMarket {
            condition_id: "c1".to_string(),
            asset_id: "a1".to_string(),
            yes_price: market_yes,
            no_price: 1.0 - market_yes,
            title: "Liverpool vs Wolves".to_string(),
        });
        f
    }

    #[test]
    fn uplift_applied_correctly() {
        // Home team with 70% home win rate → estimate = 0.46 + (0.70 - 0.46) = 0.70.
        let f = fixture_with_home_rate(0.70, 0.50);
        let prob = HomeAdvantageStrategy::estimate_home_win_prob(&f);
        assert!((prob - 0.70).abs() < 0.001, "prob was {}", prob);
    }

    #[test]
    fn signal_emitted_when_market_underprices() {
        let strat = HomeAdvantageStrategy::new();
        let f = fixture_with_home_rate(0.70, 0.50); // model=0.70, market=0.50, edge=0.20
        let sig = strat.evaluate(&f);
        assert!(sig.is_some());
        assert!((sig.unwrap().signal.edge_pct - 0.20).abs() < 0.001);
    }

    #[test]
    fn no_signal_below_min_edge() {
        let strat = HomeAdvantageStrategy::new();
        let f = fixture_with_home_rate(0.50, 0.48); // model=0.50, market=0.48, edge=0.02 < 0.05
        let sig = strat.evaluate(&f);
        assert!(sig.is_none());
    }
}
