# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## AI Assistant Rules

**All AI assistant rules are centralized in `docs/ai-rules/`.** Read and follow these files:

- **[code-owner.md](docs/ai-rules/code-owner.md)** — You own the full development lifecycle. No "TODO" or "fix later" handoffs.
- **[polymarket-arbitrage.md](docs/ai-rules/polymarket-arbitrage.md)** — Building hands-off money printers. Edge first, automate everything, data over gut, risk-aware.
- **[rust-standards.md](docs/ai-rules/rust-standards.md)** — Follow [docs/standards/STANDARDS.md](docs/standards/STANDARDS.md) for all Rust code.
- **[concise-responses.md](docs/ai-rules/concise-responses.md)** — Be concise and to-the-point. No long descriptions.
- **[plans-contain-only-plan.md](docs/ai-rules/plans-contain-only-plan.md)** — Plan documents contain only concrete steps. No "TBD" sections.
- **[visual-and-themes.md](docs/ai-rules/visual-and-themes.md)** — When changing TUI colors, apply to ALL theme presets.

## Project Context

**Polymarket Arbitrage Terminal** — Rust TUI for automated Polymarket trading across crypto, sports, and weather markets.

**Stack:** Rust + Ratatui + polymarket-client-sdk + Tokio

**Architecture:**
- `src/main.rs` — Entry point; loads config and launches TUI
- `src/config/` — Environment and configuration loading
- `src/pm/` — Polymarket client wrapper (CLOB, Gamma, Data, WebSocket)
- `src/tui/` — Terminal UI (Ratatui); layouts, themes, app state
- `src/strategies/` — Domain-specific strategies (crypto, sports, weather)
  - Each has `data/` (external feeds) and `backtest/` subdirectories
- `src/discover/` — Market discovery and search
- `src/copy_trading/` — Copy trader monitoring and execution
- `src/trader_stats/` — Trader profile analysis
- `src/live/` — Live trading execution
- `src/shared/` — Shared utilities and types

**Key Design Patterns:**
- Domain code isolated under `src/strategies/<domain>/`
- Pluggable strategies and data sources via traits
- Paper vs Live execution mode via `EXECUTION_MODE` env var
- All credentials and config via `.env` file

## Development Commands

**Build & Run:**
```bash
cargo build                  # Debug build
cargo build --release        # Release build
cargo run                    # Run debug
cargo run --release          # Run release
```

**Test & Lint:**
```bash
cargo test                   # Run all tests
cargo test <test_name>       # Run specific test
cargo test -- --nocapture    # Show println! output
cargo clippy -- -D warnings  # Must pass before merging
cargo fmt                    # Format code
```

## Environment Setup

See [docs/setup/CREDENTIALS.md](docs/setup/CREDENTIALS.md) for required credentials:
- `PRIVATE_KEY` — Ethereum wallet private key
- `FUNDER_ADDRESS` — Polymarket proxy (Safe) address
- `EXECUTION_MODE` — `paper` (default) or `live`
- Optional: `BINANCE_API_KEY`, `COPY_TRADERS_FILE`

## Key Documentation

- **[docs/standards/STANDARDS.md](docs/standards/STANDARDS.md)** — Full Rust standards and practices
- **[docs/setup/DATA_AND_CREDENTIALS.md](docs/setup/DATA_AND_CREDENTIALS.md)** — Detailed credential setup
- **[docs/setup/POLYMARKET_SETUP.md](docs/setup/POLYMARKET_SETUP.md)** — Polymarket integration details
- **[docs/setup/GETTING_STARTED.md](docs/setup/GETTING_STARTED.md)** — Quick start guide
- **[docs/ai-rules/](docs/ai-rules/)** — AI assistant rules (shared with Cursor)
