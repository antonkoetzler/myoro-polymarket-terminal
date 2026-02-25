//! Auto-execute vs manual-confirm queue logic for sports signals.

use crate::config::ExecutionMode;
use crate::shared::execution::Executor;
use crate::shared::strategy::Side;
use crate::sports::signals::{SignalFeed, SignalStatus, SportsSignal};
use std::fs::{self, OpenOptions};
use std::io::Write;

const PAPER_SPORTS_TRADES: &str = "data/paper_sports_trades.jsonl";

/// Process a new signal: auto-execute if configured, otherwise queue as Pending.
pub fn process_signal(feed: &SignalFeed, mut sig: SportsSignal, mode: ExecutionMode) {
    if sig.signal.auto_execute {
        let executor = Executor::new(mode);
        let amount = (sig.signal.kelly_size * 1000.0).max(1.0); // default bankroll $1000
        if executor
            .execute(&sig.signal.market_id, sig.signal.side, amount)
            .is_ok()
        {
            sig.status = SignalStatus::AutoExecuted;
            if !executor.is_live() {
                let _ = append_paper_trade(&sig);
            }
        }
    }
    feed.push(sig);
}

/// Execute a pending signal from the feed at index `idx`.
/// Returns true if the signal was found and executed.
pub fn execute_at(feed: &SignalFeed, idx: usize, mode: ExecutionMode) -> bool {
    let Ok(mut g) = feed.signals.write() else {
        return false;
    };
    let Some(sig) = g.get_mut(idx) else {
        return false;
    };
    if sig.status != SignalStatus::Pending {
        return false;
    }
    let executor = Executor::new(mode);
    let amount = (sig.signal.kelly_size * 1000.0).max(1.0);
    if executor
        .execute(&sig.signal.market_id, sig.signal.side, amount)
        .is_ok()
    {
        sig.status = SignalStatus::Executed;
        if !executor.is_live() {
            let _ = append_paper_trade(sig);
        }
        true
    } else {
        false
    }
}

/// Dismiss a pending signal at index `idx`.
pub fn dismiss_at(feed: &SignalFeed, idx: usize) -> bool {
    let Ok(mut g) = feed.signals.write() else {
        return false;
    };
    if let Some(sig) = g.get_mut(idx) {
        if sig.status == SignalStatus::Pending {
            sig.status = SignalStatus::Dismissed;
            return true;
        }
    }
    false
}

fn append_paper_trade(sig: &SportsSignal) -> anyhow::Result<()> {
    let dir = std::path::Path::new("data");
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }
    let side = match sig.signal.side {
        Side::Yes => "YES",
        Side::No => "NO",
    };
    let entry = serde_json::json!({
        "ts": sig.created_at.to_rfc3339(),
        "market_id": sig.signal.market_id,
        "side": side,
        "confidence": sig.signal.confidence,
        "edge_pct": sig.signal.edge_pct,
        "kelly_size": sig.signal.kelly_size,
        "strategy_id": sig.signal.strategy_id,
        "home": sig.fixture.fixture.home,
        "away": sig.fixture.fixture.away,
        "date": sig.fixture.fixture.date,
        "mode": "paper",
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(PAPER_SPORTS_TRADES)?;
    writeln!(file, "{}", entry)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::strategy::{Side, Signal};
    use crate::sports::data::Fixture;
    use crate::sports::discovery::FixtureWithStats;
    use crate::sports::signals::SportsSignal;

    fn make_signal(auto_execute: bool) -> SportsSignal {
        let fixture = FixtureWithStats::from_fixture(Fixture {
            date: "2025-03-01".to_string(),
            home: "Arsenal".to_string(),
            away: "Chelsea".to_string(),
            home_goals: None,
            away_goals: None,
        });
        SportsSignal::new(
            Signal {
                market_id: "test-market".to_string(),
                side: Side::Yes,
                confidence: 0.70,
                edge_pct: 0.10,
                kelly_size: 0.05,
                auto_execute,
                strategy_id: "poisson".to_string(),
                metadata: None,
            },
            fixture,
        )
    }

    #[test]
    fn manual_signal_stays_pending() {
        let feed = SignalFeed::default();
        let sig = make_signal(false);
        process_signal(&feed, sig, ExecutionMode::Paper);
        let g = feed.signals.read().unwrap();
        assert_eq!(g[0].status, SignalStatus::Pending);
    }

    #[test]
    fn auto_execute_fires_immediately() {
        let feed = SignalFeed::default();
        let sig = make_signal(true);
        process_signal(&feed, sig, ExecutionMode::Paper);
        let g = feed.signals.read().unwrap();
        assert_eq!(g[0].status, SignalStatus::AutoExecuted);
    }

    #[test]
    fn execute_at_changes_status() {
        let feed = SignalFeed::default();
        let sig = make_signal(false);
        feed.push(sig);
        let result = execute_at(&feed, 0, ExecutionMode::Paper);
        assert!(result);
        let g = feed.signals.read().unwrap();
        assert_eq!(g[0].status, SignalStatus::Executed);
    }

    #[test]
    fn dismiss_at_changes_status() {
        let feed = SignalFeed::default();
        let sig = make_signal(false);
        feed.push(sig);
        assert!(dismiss_at(&feed, 0));
        let g = feed.signals.read().unwrap();
        assert_eq!(g[0].status, SignalStatus::Dismissed);
    }
}
