//! Runtime TOML custom strategy loader.
//!
//! Loads `strategies/*.toml` files at startup. Each file defines conditions
//! evaluated against FixtureWithStats fields. See docs/standards/SPORTS_STRATEGY_EXTENSION.md.

use crate::shared::strategy::{Side, Signal};
use crate::sports::discovery::FixtureWithStats;
use crate::sports::signals::SportsSignal;
use crate::sports::strategies::SportsStrategy;
use anyhow::{Context, Result};
use serde::Deserialize;

/// Parsed TOML strategy file.
#[derive(Debug, Deserialize)]
struct TomlStrategyFile {
    strategy: StrategyMeta,
    #[serde(default)]
    conditions: Vec<Condition>,
}

#[derive(Debug, Deserialize)]
struct StrategyMeta {
    name: String,
    id: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    auto_execute: bool,
    #[serde(default = "default_kelly")]
    kelly_fraction: f64,
    #[serde(default = "default_min_edge")]
    min_edge: f64,
}

fn default_kelly() -> f64 {
    0.25
}
fn default_min_edge() -> f64 {
    0.05
}

#[derive(Debug, Deserialize)]
struct Condition {
    field: String,
    operator: String,
    value: serde_json::Value,
}

impl Condition {
    fn evaluate(&self, f: &FixtureWithStats) -> bool {
        let field_val = match self.field.as_str() {
            "home_win_rate" => f.home_win_rate,
            "away_win_rate" => f.away_win_rate,
            "home_xg_per90" => f.home_xg_per_90,
            "away_xg_per90" => f.away_xg_per_90,
            "market_yes_price" => f.polymarket.as_ref().map(|m| m.yes_price).unwrap_or(0.5),
            "market_no_price" => f.polymarket.as_ref().map(|m| m.no_price).unwrap_or(0.5),
            _ => return false,
        };

        match self.operator.as_str() {
            ">" => self.value.as_f64().map(|v| field_val > v).unwrap_or(false),
            "<" => self.value.as_f64().map(|v| field_val < v).unwrap_or(false),
            ">=" => self.value.as_f64().map(|v| field_val >= v).unwrap_or(false),
            "<=" => self.value.as_f64().map(|v| field_val <= v).unwrap_or(false),
            "==" => self
                .value
                .as_f64()
                .map(|v| (field_val - v).abs() < 1e-9)
                .unwrap_or(false),
            "in" => {
                // For numeric fields with array values: check if field_val is within the array.
                if let Some(arr) = self.value.as_array() {
                    arr.iter().any(|v| {
                        v.as_f64()
                            .map(|n| (field_val - n).abs() < 1e-9)
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn evaluate_string(&self, f: &FixtureWithStats) -> bool {
        if self.operator != "in" {
            return self.evaluate(f);
        }
        let field_str = match self.field.as_str() {
            "league" => f.fixture.home.as_str(), // placeholder — league not in fixture yet
            "home_team" => f.fixture.home.as_str(),
            "away_team" => f.fixture.away.as_str(),
            _ => return self.evaluate(f),
        };
        if let Some(arr) = self.value.as_array() {
            arr.iter().any(|v| v.as_str() == Some(field_str))
        } else {
            false
        }
    }
}

/// A loaded TOML custom strategy.
pub struct TomlStrategy {
    id: String,
    name: String,
    enabled: bool,
    auto_execute: bool,
    kelly_fraction: f64,
    min_edge: f64,
    conditions: Vec<Condition>,
}

impl TomlStrategy {
    pub fn from_str(toml_content: &str) -> Result<Self> {
        let file: TomlStrategyFile = toml::from_str(toml_content).context("parse TOML strategy")?;
        Ok(Self {
            id: file.strategy.id,
            name: file.strategy.name,
            enabled: file.strategy.enabled,
            auto_execute: file.strategy.auto_execute,
            kelly_fraction: file.strategy.kelly_fraction.clamp(0.01, 1.0),
            min_edge: file.strategy.min_edge.clamp(0.001, 1.0),
            conditions: file.conditions,
        })
    }

    fn all_conditions_met(&self, f: &FixtureWithStats) -> bool {
        self.conditions.iter().all(|c| match c.field.as_str() {
            "league" | "home_team" | "away_team" => c.evaluate_string(f),
            _ => c.evaluate(f),
        })
    }
}

impl SportsStrategy for TomlStrategy {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        "Custom TOML strategy"
    }
    fn is_custom(&self) -> bool {
        true
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
            .filter(|f| f.polymarket.is_some())
            .filter(|f| self.all_conditions_met(f))
            .map(|f| {
                let market = f.polymarket.as_ref().unwrap();
                let edge = (f.home_win_rate - market.yes_price).max(0.0);
                let kelly = crate::sports::strategies::poisson::kelly_fraction(
                    f.home_win_rate,
                    market.yes_price,
                ) * self.kelly_fraction;
                SportsSignal::new(
                    Signal {
                        market_id: market.asset_id.clone(),
                        side: Side::Yes,
                        confidence: f.home_win_rate,
                        edge_pct: edge,
                        kelly_size: kelly.max(0.0),
                        auto_execute: self.auto_execute,
                        strategy_id: self.id.clone(),
                        metadata: Some(serde_json::json!({
                            "home": f.fixture.home,
                            "away": f.fixture.away,
                            "strategy": self.name,
                        })),
                    },
                    f.clone(),
                )
            })
            .filter(|s| s.signal.edge_pct >= self.min_edge)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_TOML: &str = r#"
[strategy]
name = "My Home Edge"
id = "custom_home_edge"
enabled = false
auto_execute = false
kelly_fraction = 0.25
min_edge = 0.05

[[conditions]]
field = "home_win_rate"
operator = ">"
value = 0.65

[[conditions]]
field = "market_yes_price"
operator = "<"
value = 0.75
"#;

    #[test]
    fn parse_valid_toml() {
        let strat = TomlStrategy::from_str(VALID_TOML).expect("should parse");
        assert_eq!(strat.id(), "custom_home_edge");
        assert_eq!(strat.name(), "My Home Edge");
        assert!(!strat.enabled());
        assert_eq!(strat.conditions.len(), 2);
    }

    #[test]
    fn condition_evaluation_gt_passes() {
        let strat = TomlStrategy::from_str(VALID_TOML).unwrap();
        let cond = &strat.conditions[0]; // home_win_rate > 0.65

        use crate::sports::data::Fixture;
        let mut f = crate::sports::discovery::FixtureWithStats::from_fixture(Fixture {
            date: "2025-03-01".to_string(),
            home: "Arsenal".to_string(),
            away: "Chelsea".to_string(),
            home_goals: None,
            away_goals: None,
        });
        f.home_win_rate = 0.70;
        assert!(cond.evaluate(&f), "0.70 > 0.65 should pass");

        f.home_win_rate = 0.60;
        assert!(!cond.evaluate(&f), "0.60 > 0.65 should fail");
    }

    #[test]
    fn parse_invalid_toml_returns_error() {
        let result = TomlStrategy::from_str("not valid toml {{{{");
        assert!(result.is_err());
    }
}
