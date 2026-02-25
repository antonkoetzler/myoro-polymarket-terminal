//! Sports signal types and feed state.

pub mod queue;

use crate::shared::strategy::Signal;
use crate::sports::discovery::FixtureWithStats;
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Lifecycle status of a sports signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SignalStatus {
    /// Waiting for user to execute or dismiss.
    Pending,
    /// Executed automatically (auto_execute=true).
    AutoExecuted,
    /// Dismissed by user.
    Dismissed,
    /// Manually executed by user.
    Executed,
}

impl SignalStatus {
    pub fn label(self) -> &'static str {
        match self {
            SignalStatus::Pending => "pending",
            SignalStatus::AutoExecuted => "auto",
            SignalStatus::Dismissed => "dismissed",
            SignalStatus::Executed => "done",
        }
    }
}

/// A sports-domain signal enriched with fixture context and status.
#[derive(Clone, Debug)]
pub struct SportsSignal {
    pub signal: Signal,
    pub fixture: FixtureWithStats,
    pub status: SignalStatus,
    pub created_at: DateTime<Utc>,
}

impl SportsSignal {
    pub fn new(signal: Signal, fixture: FixtureWithStats) -> Self {
        Self {
            signal,
            fixture,
            status: SignalStatus::Pending,
            created_at: Utc::now(),
        }
    }

    pub fn is_pending(&self) -> bool {
        self.status == SignalStatus::Pending
    }
}

/// Thread-safe feed of sports signals.
#[derive(Default)]
pub struct SignalFeed {
    pub signals: std::sync::RwLock<Vec<SportsSignal>>,
}

impl SignalFeed {
    pub fn push(&self, signal: SportsSignal) {
        if let Ok(mut g) = self.signals.write() {
            g.push(signal);
        }
    }

    pub fn len(&self) -> usize {
        self.signals.read().map(|g| g.len()).unwrap_or(0)
    }

    pub fn pending_count(&self) -> usize {
        self.signals
            .read()
            .map(|g| g.iter().filter(|s| s.is_pending()).count())
            .unwrap_or(0)
    }
}
