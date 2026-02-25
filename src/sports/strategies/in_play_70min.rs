//! 70-minute tie rule: detect late-game value when a losing team can still equalise.
//!
//! Conditions: minute 65–85, score difference is exactly 1, Sofascore live data available.
//! Uses time-adjusted Poisson to estimate P(losing team scores ≥1 in remaining time).

use crate::shared::strategy::{Side, Signal};
use crate::sports::data::live_scores::{LiveMatchState, MatchStatus};
use crate::sports::discovery::FixtureWithStats;
use crate::sports::signals::SportsSignal;
use crate::sports::strategies::{poisson::poisson_pmf, SportsStrategy};

const STRATEGY_ID: &str = "in_play_70min";
const STRATEGY_NAME: &str = "70-Min Tie Rule";
const STRATEGY_DESC: &str =
    "Late-game xG value: losing team 1 goal down at 65-85 min. Time-adjusted Poisson.";

const MIN_MINUTE: u8 = 65;
const MAX_MINUTE: u8 = 85;
const MIN_EDGE: f64 = 0.08;
const KELLY_FRACTION: f64 = 0.25;

pub struct InPlay70MinStrategy {
    enabled: bool,
    auto_execute: bool,
    live_matches: std::sync::RwLock<Vec<LiveMatchState>>,
}

impl InPlay70MinStrategy {
    pub fn new() -> Self {
        Self {
            enabled: false,
            auto_execute: false,
            live_matches: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Update the live match cache (called by background thread every 60s).
    pub fn update_live_matches(&self, matches: Vec<LiveMatchState>) {
        if let Ok(mut g) = self.live_matches.write() {
            *g = matches;
        }
    }
}

impl SportsStrategy for InPlay70MinStrategy {
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
        let live = match self.live_matches.read() {
            Ok(g) => g.clone(),
            Err(_) => return Vec::new(),
        };
        if live.is_empty() {
            return Vec::new();
        }
        live.iter()
            .filter(|lm| qualifies(lm))
            .filter_map(|lm| {
                // Match live state to a fixture by team name.
                let fixture = fixtures.iter().find(|f| {
                    names_match(&f.fixture.home, &lm.home_team)
                        && names_match(&f.fixture.away, &lm.away_team)
                })?;
                self.evaluate(lm, fixture)
            })
            .collect()
    }
}

impl InPlay70MinStrategy {
    fn evaluate(&self, lm: &LiveMatchState, f: &FixtureWithStats) -> Option<SportsSignal> {
        let market = f.polymarket.as_ref()?;
        let minutes_played = lm.minute as f64;
        let minutes_remaining = (90.0 - minutes_played).max(1.0);

        // Determine which team is losing and their xG rate.
        let (losing_xg_per_90, _winning_xga_per_90) = if lm.home_goals < lm.away_goals {
            // Home is losing.
            (f.home_xg_per_90, f.away_xga_per_90)
        } else {
            // Away is losing.
            (f.away_xg_per_90, f.home_xga_per_90)
        };

        // Time-adjusted lambda: scale xG by fraction of game remaining.
        let lambda = losing_xg_per_90 * (minutes_remaining / 90.0);

        // P(losing team scores ≥1 goal) = 1 - P(0 goals).
        let p_score_at_least_one = 1.0 - poisson_pmf(lambda, 0);

        // Draw market: this is a "will the game end in a draw?" market.
        // We use no_price as draw probability proxy (rough approximation).
        let market_draw_price = market.no_price;
        let edge = p_score_at_least_one - market_draw_price;

        if edge < MIN_EDGE {
            return None;
        }

        let exit_target = p_score_at_least_one * 0.85; // suggested early-sell level

        Some(SportsSignal::new(
            Signal {
                market_id: market.asset_id.clone(),
                side: Side::No, // betting on draw/comeback
                confidence: p_score_at_least_one,
                edge_pct: edge,
                kelly_size: (p_score_at_least_one - market_draw_price) / (1.0 - market_draw_price)
                    * KELLY_FRACTION,
                auto_execute: self.auto_execute,
                strategy_id: STRATEGY_ID.to_string(),
                metadata: Some(serde_json::json!({
                    "home": lm.home_team,
                    "away": lm.away_team,
                    "score": format!("{}-{}", lm.home_goals, lm.away_goals),
                    "minute": lm.minute,
                    "lambda": lambda,
                    "p_equalise": p_score_at_least_one,
                    "market_draw_price": market_draw_price,
                    "exit_target": exit_target,
                    "note": "Live: season-avg xG used (not per-match); Sofascore data",
                })),
            },
            f.clone(),
        ))
    }
}

/// Check if a live match meets the 70-min rule entry conditions.
fn qualifies(lm: &LiveMatchState) -> bool {
    lm.status == MatchStatus::InPlay
        && lm.minute >= MIN_MINUTE
        && lm.minute <= MAX_MINUTE
        && lm.home_goals.abs_diff(lm.away_goals) == 1
}

fn names_match(fixture_name: &str, live_name: &str) -> bool {
    let a = fixture_name.to_lowercase();
    let b = live_name.to_lowercase();
    a.contains(&b) || b.contains(&a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sports::data::{live_scores::MatchStatus, Fixture};
    use crate::sports::discovery::PolymarketMarket;

    fn make_live_match(minute: u8, home_goals: u8, away_goals: u8) -> LiveMatchState {
        LiveMatchState {
            match_id: "live1".to_string(),
            home_team: "Arsenal".to_string(),
            away_team: "Chelsea".to_string(),
            home_goals,
            away_goals,
            minute,
            status: MatchStatus::InPlay,
        }
    }

    fn make_fixture() -> FixtureWithStats {
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
            yes_price: 0.75,
            no_price: 0.25,
            title: "Arsenal vs Chelsea".to_string(),
        });
        f
    }

    #[test]
    fn no_signal_before_minute_65() {
        let lm = make_live_match(60, 0, 1);
        assert!(!qualifies(&lm), "minute 60 should not qualify");
    }

    #[test]
    fn no_signal_when_two_goal_difference() {
        let lm = make_live_match(75, 0, 2);
        assert!(!qualifies(&lm), "2-goal deficit should not qualify");
    }

    #[test]
    fn qualifies_at_minute_75_one_goal_down() {
        let lm = make_live_match(75, 0, 1);
        assert!(qualifies(&lm));
    }

    #[test]
    fn draw_probability_increases_as_time_decreases() {
        // More time remaining → higher chance of equalising.
        let lambda_early = 1.4 * (20.0 / 90.0); // 70 min, 20 remaining
        let lambda_late = 1.4 * (10.0 / 90.0); // 80 min, 10 remaining

        let p_early = 1.0 - poisson_pmf(lambda_early, 0);
        let p_late = 1.0 - poisson_pmf(lambda_late, 0);
        assert!(
            p_early > p_late,
            "earlier = more time = higher equalise prob"
        );
    }
}
