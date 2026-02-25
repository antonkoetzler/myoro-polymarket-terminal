//! 1.20 Rule: systematic low-odds value detection for heavy favourites.
//!
//! Targets Polymarket markets where the favourite's implied probability is 0.80–0.87
//! (~1.15–1.25 decimal odds) AND our Poisson model agrees on >80% win probability.

use crate::shared::strategy::{Side, Signal};
use crate::sports::discovery::FixtureWithStats;
use crate::sports::signals::SportsSignal;
use crate::sports::strategies::{
    poisson::{kelly_fraction, match_probs},
    SportsStrategy,
};

const STRATEGY_ID: &str = "rule_1_20";
const STRATEGY_NAME: &str = "1.20 Rule";
const STRATEGY_DESC: &str =
    "Value-bets on heavy favourites (market 0.80–0.87) confirmed by Poisson model. Min $5 Kelly.";

const PROB_LOW: f64 = 0.80;
const PROB_HIGH: f64 = 0.87;
const MODEL_MIN_PROB: f64 = 0.80;
const MIN_KELLY_FRACTION: f64 = 0.005; // $5 minimum on a $1000 bankroll
const KELLY_FRACTION: f64 = 0.25;
const MIN_EDGE: f64 = 0.02; // smaller edge is fine when the favourite is near-certain

pub struct Rule120Strategy {
    enabled: bool,
    auto_execute: bool,
}

impl Rule120Strategy {
    pub fn new() -> Self {
        Self {
            enabled: false,
            auto_execute: false,
        }
    }
}

impl SportsStrategy for Rule120Strategy {
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

impl Rule120Strategy {
    fn evaluate(&self, f: &FixtureWithStats) -> Option<SportsSignal> {
        let market = f.polymarket.as_ref()?;

        // Check if market has a heavy favourite.
        let (fav_side, fav_price, model_prob) =
            if market.yes_price >= PROB_LOW && market.yes_price <= PROB_HIGH {
                let (p_home, _, _) = match_probs(
                    f.home_xg_per_90,
                    f.away_xg_per_90,
                    f.home_xga_per_90,
                    f.away_xga_per_90,
                );
                (Side::Yes, market.yes_price, p_home)
            } else if market.no_price >= PROB_LOW && market.no_price <= PROB_HIGH {
                let (_, _, p_away) = match_probs(
                    f.home_xg_per_90,
                    f.away_xg_per_90,
                    f.home_xga_per_90,
                    f.away_xga_per_90,
                );
                (Side::No, market.no_price, p_away)
            } else {
                return None;
            };

        // Model must also agree the favourite has >80% probability.
        if model_prob < MODEL_MIN_PROB {
            return None;
        }

        let edge = model_prob - fav_price;
        if edge < MIN_EDGE {
            return None;
        }

        let kelly = kelly_fraction(model_prob, fav_price) * KELLY_FRACTION;
        if kelly < MIN_KELLY_FRACTION {
            return None; // below minimum bet size
        }

        Some(SportsSignal::new(
            Signal {
                market_id: market.asset_id.clone(),
                side: fav_side,
                confidence: model_prob,
                edge_pct: edge,
                kelly_size: kelly,
                auto_execute: self.auto_execute,
                strategy_id: STRATEGY_ID.to_string(),
                metadata: Some(serde_json::json!({
                    "home": f.fixture.home,
                    "away": f.fixture.away,
                    "fav_market_price": fav_price,
                    "model_prob": model_prob,
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

    fn fixture_with_market(yes_price: f64, home_xg: f64) -> FixtureWithStats {
        let mut f = crate::sports::discovery::FixtureWithStats::from_fixture(Fixture {
            date: "2025-03-01".to_string(),
            home: "Bayern".to_string(),
            away: "Heidenheim".to_string(),
            home_goals: None,
            away_goals: None,
        });
        f.home_xg_per_90 = home_xg;
        f.away_xg_per_90 = 0.6;
        f.home_xga_per_90 = 0.8;
        f.away_xga_per_90 = 2.2;
        f.polymarket = Some(PolymarketMarket {
            condition_id: "c1".to_string(),
            asset_id: "a1".to_string(),
            yes_price,
            no_price: 1.0 - yes_price,
            title: "Bayern vs Heidenheim".to_string(),
        });
        f
    }

    #[test]
    fn kelly_size_calculation_is_correct() {
        // p=0.85, price=0.83 → b = 0.17/0.83 = 0.205, kelly = (0.85*0.205 - 0.15) / 0.205 ≈ 0.115
        let k = kelly_fraction(0.85, 0.83);
        assert!(k > 0.0 && k < 1.0, "kelly={}", k);
        let full_k = kelly_fraction(0.85, 0.83);
        assert!((full_k - 0.115).abs() < 0.01, "full kelly={}", full_k);
    }

    #[test]
    fn signal_emitted_for_heavy_favourite() {
        let strat = Rule120Strategy::new();
        // Market at 0.83 (within 0.80–0.87 band).
        // Strong Bayern xG should make model agree >80%.
        let f = fixture_with_market(0.83, 2.8);
        let sig = strat.evaluate(&f);
        assert!(sig.is_some(), "expected signal for heavy favourite");
    }

    #[test]
    fn filters_below_odds_threshold() {
        let strat = Rule120Strategy::new();
        // Market at 0.60 (1.67 odds) → below the 0.80–0.87 band.
        let f = fixture_with_market(0.60, 1.5);
        let sig = strat.evaluate(&f);
        assert!(sig.is_none(), "expected no signal when outside 1.20 range");
    }

    #[test]
    fn filters_above_odds_threshold() {
        let strat = Rule120Strategy::new();
        // Market at 0.92 → above the 0.80–0.87 band.
        let f = fixture_with_market(0.92, 2.8);
        let sig = strat.evaluate(&f);
        assert!(sig.is_none(), "expected no signal when above 0.87 band");
    }
}
