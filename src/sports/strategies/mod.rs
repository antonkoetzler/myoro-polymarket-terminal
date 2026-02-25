//! Sports strategy registry: loads built-in strategies + optional TOML custom strategies.

pub mod arb_scanner;
pub mod home_advantage;
pub mod in_play_70min;
pub mod poisson;
pub mod rule_1_20;
pub mod toml_strategy;

use crate::sports::discovery::FixtureWithStats;
use crate::sports::signals::SportsSignal;

/// Common interface for all sports betting strategies.
pub trait SportsStrategy: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    /// Brief description shown in TUI strategies pane.
    fn description(&self) -> &str;
    fn is_custom(&self) -> bool;
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, v: bool);
    fn auto_execute(&self) -> bool;
    /// Scan fixtures and return signals.
    fn scan(&self, fixtures: &[FixtureWithStats]) -> Vec<SportsSignal>;
}

/// Strategy registry: holds all enabled strategies and runs scans.
pub struct StrategyRegistry {
    strategies: Vec<Box<dyn SportsStrategy>>,
}

impl Default for StrategyRegistry {
    fn default() -> Self {
        let mut reg = Self {
            strategies: Vec::new(),
        };
        reg.load_builtins();
        reg
    }
}

impl StrategyRegistry {
    fn load_builtins(&mut self) {
        self.strategies
            .push(Box::new(poisson::PoissonStrategy::new()));
        self.strategies
            .push(Box::new(home_advantage::HomeAdvantageStrategy::new()));
        self.strategies
            .push(Box::new(rule_1_20::Rule120Strategy::new()));
        self.strategies
            .push(Box::new(arb_scanner::ArbScannerStrategy::new()));
        self.strategies
            .push(Box::new(in_play_70min::InPlay70MinStrategy::new()));

        // Load TOML custom strategies from strategies/ directory if it exists.
        if let Ok(entries) = std::fs::read_dir("strategies") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(strat) = toml_strategy::TomlStrategy::from_str(&content) {
                            self.strategies.push(Box::new(strat));
                        }
                    }
                }
            }
        }
    }

    /// Iterate over all strategies (enabled and disabled).
    pub fn all(&self) -> &[Box<dyn SportsStrategy>] {
        &self.strategies
    }

    /// Toggle a strategy by id. Returns the new enabled state, or None if not found.
    pub fn toggle(&mut self, id: &str) -> Option<bool> {
        self.strategies.iter_mut().find(|s| s.id() == id).map(|s| {
            let new_state = !s.enabled();
            s.set_enabled(new_state);
            new_state
        })
    }

    /// Run all enabled strategies over the given fixtures and collect signals.
    pub fn scan(&self, fixtures: &[FixtureWithStats]) -> Vec<SportsSignal> {
        self.strategies
            .iter()
            .filter(|s| s.enabled())
            .flat_map(|s| s.scan(fixtures))
            .collect()
    }
}
