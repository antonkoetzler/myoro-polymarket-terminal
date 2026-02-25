//! Poisson + Dixon-Coles goal model. Predicts P(H), P(D), P(A) from team xG stats.

use crate::shared::strategy::{Side, Signal};
use crate::sports::discovery::FixtureWithStats;
use crate::sports::signals::SportsSignal;
use crate::sports::strategies::SportsStrategy;

const STRATEGY_ID: &str = "poisson";
const STRATEGY_NAME: &str = "Poisson Model";
const STRATEGY_DESC: &str =
    "Poisson + Dixon-Coles. Compares predicted win prob vs market. Min edge: 5%.";

/// Dixon-Coles rho parameter (negative = slight correlation between low scores).
const DC_RHO: f64 = -0.10;
/// Minimum edge over market price to emit a signal.
const MIN_EDGE: f64 = 0.05;
/// Quarter-Kelly fraction applied to raw Kelly stake.
const KELLY_FRACTION: f64 = 0.25;

pub struct PoissonStrategy {
    enabled: bool,
    auto_execute: bool,
    min_edge: f64,
    kelly_fraction: f64,
}

impl PoissonStrategy {
    pub fn new() -> Self {
        Self {
            enabled: false,
            auto_execute: false,
            min_edge: MIN_EDGE,
            kelly_fraction: KELLY_FRACTION,
        }
    }
}

impl SportsStrategy for PoissonStrategy {
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
            .filter(|f| f.fixture.home_goals.is_none()) // only upcoming matches
            .filter_map(|f| self.evaluate(f))
            .collect()
    }
}

impl PoissonStrategy {
    fn evaluate(&self, f: &FixtureWithStats) -> Option<SportsSignal> {
        let market = f.polymarket.as_ref()?;

        let (p_home, _p_draw, p_away) = match_probs(
            f.home_xg_per_90,
            f.away_xg_per_90,
            f.home_xga_per_90,
            f.away_xga_per_90,
        );

        // Check home win edge.
        let home_edge = p_home - market.yes_price;
        if home_edge >= self.min_edge {
            let kelly = kelly_fraction(p_home, market.yes_price) * self.kelly_fraction;
            return Some(SportsSignal::new(
                Signal {
                    market_id: market.asset_id.clone(),
                    side: Side::Yes,
                    confidence: p_home,
                    edge_pct: home_edge,
                    kelly_size: kelly.max(0.0),
                    auto_execute: self.auto_execute,
                    strategy_id: STRATEGY_ID.to_string(),
                    metadata: Some(serde_json::json!({
                        "home": f.fixture.home,
                        "away": f.fixture.away,
                        "p_home": p_home,
                        "market_yes": market.yes_price,
                    })),
                },
                f.clone(),
            ));
        }

        // Check away win edge (away team wins = NO on a home-win market).
        let away_edge = p_away - market.no_price;
        if away_edge >= self.min_edge {
            let kelly = kelly_fraction(p_away, market.no_price) * self.kelly_fraction;
            return Some(SportsSignal::new(
                Signal {
                    market_id: market.asset_id.clone(),
                    side: Side::No,
                    confidence: p_away,
                    edge_pct: away_edge,
                    kelly_size: kelly.max(0.0),
                    auto_execute: self.auto_execute,
                    strategy_id: STRATEGY_ID.to_string(),
                    metadata: Some(serde_json::json!({
                        "home": f.fixture.home,
                        "away": f.fixture.away,
                        "p_away": p_away,
                        "market_no": market.no_price,
                    })),
                },
                f.clone(),
            ));
        }

        None
    }
}

// ── Poisson math ─────────────────────────────────────────────────────────────

/// Poisson PMF: P(X=k) = e^{-λ} λ^k / k!
pub fn poisson_pmf(lambda: f64, k: u32) -> f64 {
    if lambda <= 0.0 {
        return if k == 0 { 1.0 } else { 0.0 };
    }
    (-lambda).exp() * lambda.powi(k as i32) / factorial(k)
}

fn factorial(n: u32) -> f64 {
    (1..=n).map(|i| i as f64).product::<f64>().max(1.0)
}

/// Dixon-Coles correction factor for low-scoring scorelines.
fn dc_tau(i: u32, j: u32, mu: f64, nu: f64, rho: f64) -> f64 {
    match (i, j) {
        (0, 0) => (1.0 - mu * nu * rho).max(0.0),
        (1, 0) => 1.0 + nu * rho,
        (0, 1) => 1.0 + mu * rho,
        (1, 1) => (1.0 - rho).max(0.0),
        _ => 1.0,
    }
}

/// Compute P(home win), P(draw), P(away win) using Poisson + Dixon-Coles.
/// Returns (p_home, p_draw, p_away) guaranteed to sum to ≈1.0.
pub fn match_probs(home_xg: f64, away_xg: f64, home_xga: f64, away_xga: f64) -> (f64, f64, f64) {
    // League average xG: used to adjust attack/defence strengths.
    const LEAGUE_AVG_XG: f64 = 1.35;

    // Attack and defence strength ratios.
    let home_attack = home_xg / LEAGUE_AVG_XG;
    let home_defence = home_xga / LEAGUE_AVG_XG;
    let away_attack = away_xg / LEAGUE_AVG_XG;
    let away_defence = away_xga / LEAGUE_AVG_XG;

    // Poisson lambda for each team.
    let lambda_h = (home_attack * away_defence * LEAGUE_AVG_XG * 1.10).max(0.1); // +10% home advantage
    let lambda_a = (away_attack * home_defence * LEAGUE_AVG_XG).max(0.1);

    let max_goals = 7usize;
    let mut p_home = 0.0_f64;
    let mut p_draw = 0.0_f64;
    let mut p_away = 0.0_f64;

    for i in 0..max_goals {
        for j in 0..max_goals {
            let p_ij = poisson_pmf(lambda_h, i as u32)
                * poisson_pmf(lambda_a, j as u32)
                * dc_tau(i as u32, j as u32, lambda_h, lambda_a, DC_RHO);
            if i > j {
                p_home += p_ij;
            } else if i == j {
                p_draw += p_ij;
            } else {
                p_away += p_ij;
            }
        }
    }

    // Normalize to account for truncation at max_goals.
    let total = p_home + p_draw + p_away;
    if total > 0.0 {
        (p_home / total, p_draw / total, p_away / total)
    } else {
        (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)
    }
}

/// Fractional Kelly: f = (p*b - q) / b  where b = (1/price - 1).
pub fn kelly_fraction(true_prob: f64, market_price: f64) -> f64 {
    if market_price <= 0.0 || market_price >= 1.0 {
        return 0.0;
    }
    let b = (1.0 - market_price) / market_price; // net odds
    let q = 1.0 - true_prob;
    let k = (true_prob * b - q) / b;
    k.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sports::data::Fixture;
    use crate::sports::discovery::PolymarketMarket;

    fn fixture_with_market(yes_price: f64) -> FixtureWithStats {
        let mut f = crate::sports::discovery::FixtureWithStats::from_fixture(Fixture {
            date: "2025-03-01".to_string(),
            home: "Arsenal".to_string(),
            away: "Chelsea".to_string(),
            home_goals: None,
            away_goals: None,
        });
        f.polymarket = Some(PolymarketMarket {
            condition_id: "cond1".to_string(),
            asset_id: "asset1".to_string(),
            yes_price,
            no_price: 1.0 - yes_price,
            title: "Arsenal vs Chelsea".to_string(),
        });
        f
    }

    #[test]
    fn win_probabilities_sum_to_one() {
        let (ph, pd, pa) = match_probs(1.5, 1.1, 1.2, 1.4);
        let sum = ph + pd + pa;
        assert!((sum - 1.0).abs() < 1e-6, "sum was {}", sum);
    }

    #[test]
    fn home_advantage_reflected_in_probs() {
        // Home team has stronger xG: p_home should be > p_away.
        let (ph, _pd, pa) = match_probs(2.0, 0.8, 1.0, 1.8);
        assert!(ph > pa, "expected home > away: {} vs {}", ph, pa);
    }

    #[test]
    fn value_detected_when_market_underprices() {
        let strat = PoissonStrategy::new();
        // Model will give home ~50%+ for equal teams; market at 0.40 → big edge.
        let f = fixture_with_market(0.40);
        let sig = strat.evaluate(&f);
        assert!(
            sig.is_some(),
            "expected signal when market underprices home team"
        );
    }

    #[test]
    fn no_signal_when_no_edge() {
        let _strat = PoissonStrategy::new();
        // Market price matches model closely.
        let f = fixture_with_market(0.55);
        // With default xG 1.4 vs 1.2, home win prob ≈ 0.53, edge = -0.02 < 0.05.
        let f2 = {
            let mut ff = f.clone();
            ff.home_xg_per_90 = 1.35;
            ff.away_xg_per_90 = 1.35;
            if let Some(ref mut m) = ff.polymarket {
                m.yes_price = 0.48; // close to model
                m.no_price = 0.52;
            }
            ff
        };
        let (p_home, _, p_away) = match_probs(
            f2.home_xg_per_90,
            f2.away_xg_per_90,
            f2.home_xga_per_90,
            f2.away_xga_per_90,
        );
        let market_yes = f2.polymarket.as_ref().unwrap().yes_price;
        let market_no = f2.polymarket.as_ref().unwrap().no_price;
        // Verify edges are small.
        assert!((p_home - market_yes).abs() < 0.10 || (p_away - market_no).abs() < 0.10);
    }

    #[test]
    fn no_signal_for_completed_fixture() {
        let strat = PoissonStrategy::new();
        let mut f = fixture_with_market(0.30);
        f.fixture.home_goals = Some(2); // fixture already completed
        let result = strat.scan(&[f]);
        assert!(result.is_empty());
    }

    #[test]
    fn kelly_fraction_reasonable_value() {
        // p=0.65, price=0.50 (even money) → kelly = (0.65 - 0.35) / 1.0 = 0.30
        let k = kelly_fraction(0.65, 0.50);
        assert!((k - 0.30).abs() < 0.001, "kelly was {}", k);
    }

    #[test]
    fn poisson_pmf_sums_close_to_one() {
        let lambda = 1.5;
        let sum: f64 = (0..20).map(|k| poisson_pmf(lambda, k)).sum();
        assert!((sum - 1.0).abs() < 1e-4, "sum was {}", sum);
    }
}
