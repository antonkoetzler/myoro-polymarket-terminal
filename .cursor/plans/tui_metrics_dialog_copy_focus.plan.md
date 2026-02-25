# TUI Metrics, Add Trader, Copy Tab, and Section Focus (revised)

## 1. Metrics section ([src/tui/layout.rs](src/tui/layout.rs), [src/tui/app.rs](src/tui/app.rs))

- **Space after colon:** In `Layout::render`, add a space after each metrics label so display is "P&L: $0.00", "Bankroll: 10.00", etc. (e.g. render label as `format!("{} ", label)`).
- **Currency prefix for money:** Use existing `pnl_prefix` for Bankroll as well (format bankroll value as `format!("{}{:.2}", pnl_prefix, b)` when Some).
- **Copy status text and color:** Pass only "Running" / "Stopped" (not "Copy: Running") to the metrics row. When Copy tab is active, style the value green (`Theme::success()`) for Running, red (`Theme::danger()`) for Stopped. Layout needs an extra parameter for copy value style; apply to all theme presets (success/danger already in theme.rs).

**Verification:** Run app; Metrics show "Bankroll: $10.00", "Copy: Running" in green / "Stopped" in red, no "Copy:Copy:".

---

## 2. Add trader dialog ([src/tui/app.rs](src/tui/app.rs), [src/tui/layout.rs](src/tui/layout.rs))

- **List navigation:** In the `copy_add_dialog` key handler, use **only** `KeyCode::Up` and `KeyCode::Down` for list selection (remove `Char('k')` and `Char('j')`) so 'k', 'j', and all other keys go to the search input.
- **Cursor / stray "||":** Remove or replace the "▌" cursor in the Add trader search line so only one character appears (e.g. simple underscore or no cursor); avoid any double pipe rendering.

**Verification:** Add trader dialog: typing "kch123" works; no duplicate pipe.

---

## 3. Copy trading screen – remove Logs ([src/tui/layout.rs](src/tui/layout.rs))

- Use all of `chunks[3]` as `main_rect` for main tabs (no 70/30 split). Remove the Logs panel render block. Bottom strip (Copy Status, Last copy) stays on `chunks[4]`.

**Verification:** Copy tab has no Logs column; Copy List and Recent Trades use full width.

---

## 4. Copy trading – full functionality (paper + live, UI updates, tests)

**No "optional later".** Copy trading must work correctly for both Paper and Live, with UI reflecting state.

### 4a. Paper: UI updates on every paper trade

- **Cost:** For each executed paper trade, cost = `amount * price` (USD).
- **LiveState:** Add `apply_paper_trade(&self, cost_usd: f64)` in [src/live/mod.rs](src/live/mod.rs): decrement `global_stats.bankroll` by `cost_usd` (if `bankroll` is `Some`), increment `global_stats.open_trades`.
- **Copy path:** In [src/copy_trading/mod.rs](src/copy_trading/mod.rs), after a successful paper execute (and after `append_paper_trade_jsonl`), call `log_sink.apply_paper_trade(amount * r.price)` when `log_sink` is `Some`. Monitor already has `log_sink: Option<Arc<LiveState>>`.
- **Executor:** Paper mode remains no CLOB call; the copy_trading loop calls `apply_paper_trade` on the same `LiveState` used for logs.

**Verification:** Run Copy tab with one trader, copy_auto_execute true, paper mode; when a trade is copied, Metrics bankroll decreases and Open count increases. Unit test: given one paper trade execution and a LiveState with initial bankroll, assert bankroll and open_trades after `apply_paper_trade`.

### 4b. Live: Real CLOB execution

- **Current state:** [src/shared/execution.rs](src/shared/execution.rs) `Executor::execute` for Live is a no-op. [src/pm/mod.rs](src/pm/mod.rs) is a stub. Config has `PolymarketConfig` (funder_address, private_key, api_key, api_secret, api_passphrase).
- **Requirement:** When `ExecutionMode::Live`, `Executor::execute` must place an order on the Polymarket CLOB (using polymarket-client-sdk, e.g. createAndPostOrder or createOrder + postOrder). Use config credentials; if credentials are missing or invalid, return `Err` and let the copy_trading path push_copy_log so the UI shows "Copy execute failed" with a clear reason.
- **Implementation options:** (1) Executor holds or builds a CLOB client (from config) and calls SDK place-order API; (2) Inject a trait for execution so copy_trading calls the injectable implementation. Prefer (1) with a clear `anyhow::Error` on missing config or SDK failure. After a successful Live order, optionally update `global_stats` (e.g. open_trades) if a LiveState handle is available; otherwise at least log success.
- **PmClient:** Extend or replace usage so that Live execution uses the SDK (clob feature); see [Polymarket CLOB docs](https://docs.polymarket.com/developers/CLOB/orders/create-order) and `polymarket-client-sdk` crate docs for order creation/signing/posting.

**Verification:** With Live mode and valid credentials, a copied trade results in a real order (or a clear error in Logs/Copy Status if credentials/API fail). Test: unit test that Executor::execute in Live returns Err when credentials are missing; integration test with mock or sandbox if available.

### 4c. Tests

- **Paper:** Unit test that after one paper trade execution, `LiveState::apply_paper_trade(cost)` updates `global_stats.bankroll` and `open_trades` as expected. Unit test for copy_trading: with `copy_auto_execute = true`, one valid trade, and a LiveState log_sink, assert (1) paper file has one record, (2) bankroll and open_trades updated (or assert `apply_paper_trade` was invoked with correct cost). Refactor if needed (e.g. extract process-new-trades loop or pass LiveState into a testable function).
- **Executor:** Paper returns Ok and does not call CLOB; Live returns Err when credentials missing or SDK fails, and returns Ok when order is placed.
- **Integration-style:** Test that the full copy path (fetch → execute → apply_paper_trade / file append) runs correctly for paper with a test double or temp file and in-memory LiveState.

**Verification:** `cargo test` passes; new tests cover paper UI updates and Executor behavior.

---

## 5. Global: Section focus and Tab / scroll keys ([src/tui/app.rs](src/tui/app.rs), [src/tui/layout.rs](src/tui/layout.rs))

- **Tab:** Remove Tab/BackTab from tab switching. Use Tab/BackTab only to move focus between sections on the current tab. Keep E/Q and 1–5 for tab switching.
- **Section focus:** Add `focused_section: usize` and per-section scroll offsets for each tab. Sections = scrollable areas (e.g. Crypto events, Sports fixtures, Copy List, Recent Trades, Discover leaderboard).
- **Scroll keys when a section is focused:** Up/Down and k/j = line scroll; Left/Right and h/l = page scroll. Layout uses scroll offsets when rendering Paragraph/Table content.
- **Scope:** Define scrollable blocks per tab; pass focused_section and scroll offsets from app to layout; handle keys in app and pass updated offsets to layout.

**Verification:** Tab/BackTab cycle sections only; E/Q and 1–5 switch tabs; focused section scrolls with line/page keys.

---

## 6. Shortcuts and hints

- Update hints: remove Tab from "next tab"; add "Tab: focus section". Adjust "↑/↓/j/k" for Copy/Discover to reflect section focus and scroll.

---

## Implementation order (suggested)

1. Metrics (space, currency, Copy value + color)  
2. Add trader (arrows-only list, cursor fix)  
3. Remove Logs panel  
4. Copy trading: Paper UI updates (LiveState::apply_paper_trade, call from copy path)  
5. Copy trading: Live CLOB (Executor + PmClient/SDK, credentials, Err on failure)  
6. Copy trading: Tests (paper bankroll/open_trades, Executor, integration path)  
7. Section focus + Tab/scroll (state, keys, layout)  
8. Shortcuts/hints  

---

## Summary of file touches

| Area | Files |
|------|--------|
| Metrics | layout.rs (labels space, bankroll prefix, copy style), app.rs (pass Running/Stopped + style) |
| Add trader | app.rs (Up/Down only), layout.rs (cursor) |
| Logs removal | layout.rs (main_rect = full chunks[3], remove logs block) |
| Paper UI | live/mod.rs (apply_paper_trade), copy_trading/mod.rs (call after paper execute) |
| Live execution | shared/execution.rs (Live: CLOB place order), pm/mod.rs or config (SDK client), config credentials |
| Tests | copy_trading/mod.rs, live/mod.rs, shared/execution.rs |
| Section focus | app.rs (state, Tab/scroll keys), layout.rs (scroll offsets) |
