//! Execution: paper or live CLOB; rate limit and position limits from config.

use crate::config::ExecutionMode;
use anyhow::Result;

pub struct Executor {
    mode: ExecutionMode,
}

impl Executor {
    pub fn new(mode: ExecutionMode) -> Self {
        Self { mode }
    }

    pub fn execute(
        &self,
        _market_id: &str,
        _side: super::strategy::Side,
        _amount: f64,
    ) -> Result<()> {
        match self.mode {
            ExecutionMode::Paper => {
                // Paper: never call CLOB; log or store for replay only.
            }
            ExecutionMode::Live => {
                // Live: call CLOB (not implemented yet).
            }
        }
        Ok(())
    }

    pub fn is_live(&self) -> bool {
        self.mode == ExecutionMode::Live
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::strategy::Side;

    #[test]
    fn paper_executor_returns_ok_and_does_not_send() {
        let exec = Executor::new(ExecutionMode::Paper);
        assert!(!exec.is_live());
        assert!(exec.execute("market1", Side::Yes, 1.0).is_ok());
    }

    #[test]
    fn live_executor_returns_ok() {
        let exec = Executor::new(ExecutionMode::Live);
        assert!(exec.is_live());
        // When CLOB is wired: would send order; for now just returns Ok
        assert!(exec.execute("market1", Side::No, 1.0).is_ok());
    }
}
