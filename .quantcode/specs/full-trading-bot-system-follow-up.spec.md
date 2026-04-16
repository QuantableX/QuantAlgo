---
title: Full Trading Bot System - Follow Up Review
status: open
priority: critical
linkedFiles:
  - .quantcode/specs/full-trading-bot-system.spec.md
  - app/components/Deploy/Modal.vue
  - app/pages/strategies.vue
  - app/pages/index.vue
  - app/stores/bot.ts
  - app/stores/strategies.ts
  - app/stores/exchange.ts
  - app/layouts/default.vue
  - app/components/Terminal/BotTerminal.vue
  - app/components/Layout/RightSidebar.vue
  - src-tauri/src/lib.rs
  - python/quantalgo/runner.py
  - python/quantalgo/strategy.py
  - python/strategies/ema_cross.py
createdAt: 2026-04-14T00:00:00.000Z
updatedAt: 2026-04-14T00:00:00.000Z
---

## Review Verdict

- Verdict: partial implementation, not complete. Treat this as a failed attempt at the full spec until the follow-up items below are fixed.
- The agent did add a deploy modal, paper/live labels, a backend preflight command, richer bot status fields, terminal historical log loading, and some exchange pair/balance work.
- That is real progress over the previous hard-coded deploy button, but it is not enough. The implementation still has major functional gaps and a few dangerous illusions of correctness.
- Do not call this "fully functional". It is closer to a better paper-trading demo with a preflight UI than a refined trading bot system.
- Review method: static code review only. No builds, tests, or app launches were run.

## Critical Failures

- Dirty strategy handling is fake.
  - `app/pages/strategies.vue` passes `:is-dirty="isSaving"` into the deploy modal. Saving-in-progress is not the same as unsaved editor content.
  - `app/pages/index.vue` passes `:is-dirty="false"` from the dashboard, so dashboard start can never warn about unsaved strategy changes.
  - `app/stores/strategies.ts` has no baseline saved content, dirty flag, or last-saved tracking.

- Preflight is weak and partly misleading.
  - `validate_bot_deploy` only checks that a pair string is non-empty. It does not prove the selected exchange actually supports that pair.
  - `get_exchange_pairs` falls back to common hard-coded pairs when provider calls fail, so an unsupported or unreachable exchange can still look valid for paper trading.
  - The Python import check uses a lazy import pattern and does not verify there is a `Strategy` subclass. It can miss real load failures.
  - Strategy import failures are only a warning, not fatal, even though a broken strategy should block deploy.
  - It does not validate strategy params JSON, deploy config, timeframe, initial balance, risk per trade, max positions, slippage, or available balance.
  - It does not return enough stderr/stdout details to make broken strategies actionable.

- Live trading is still not implemented and the backend does not hard-block it.
  - The modal disables the Live Trading button, but `start_bot` still accepts `trading_mode = "live"` if called directly.
  - Backend "live" mode still runs the synthetic candle loop and simulated fills. It can label the run as live while not placing real orders.
  - There is no real order placement, cancel, close, fill reconciliation, precision handling, min-size validation, or sandbox/testnet gate.
  - This violates the spec requirement that real mode be clearly disabled until it is actually safe and implemented.

- The selected deploy pair does not reliably reach the Python strategy.
  - `Strategy.__init__` still defaults `_pair` to `"BTC/USDT"`.
  - `python/quantalgo/runner.py` does not set `strategy._pair` from the selected deploy pair or incoming candle pair.
  - `python/strategies/ema_cross.py` uses `pair = self._pair`, so deploying another pair such as `ETH/USDT` can still generate orders for `BTC/USDT`.
  - This is a core correctness failure, not polish.

- Strategy position and balance synchronization is still broken.
  - The Rust host sends `on_trade` notifications, but the Python runner only calls `strategy.on_trade(trade)`.
  - It does not update `strategy._positions` or `strategy._balance` after fills.
  - Strategies using `get_position()` and `get_balance()` can make decisions from stale state.
  - The EMA strategy can believe it has no position while the Rust runtime has open positions.

- Bot lifecycle hardening is incomplete.
  - `start_bot` writes `bot_state = running` and emits running before proving the child strategy loaded and stayed alive.
  - Spawn/load/runtime errors are not emitted as `bot:error` from Rust. There is no backend `bot:error` emission in the reviewed code path.
  - `stop_bot` still kills the child process instead of performing a reliable graceful stop and timeout fallback.
  - Stale running state is reset on startup, but the user gets no explanation of the previous crash/stale process.

- Logs are still not a robust observability system.
  - Logs are only in memory, not persisted to disk/session storage.
  - `start_bot` clears the in-memory log buffer, so prior preflight/start context can disappear.
  - Preflight checks are not pushed into bot logs; they only live in the modal.
  - `get_bot_logs` returns a simple slice from the start of the buffer, so `limit: 500` can return old logs instead of the newest logs when the buffer grows.
  - Right-sidebar terminal log filters still do not filter the actual terminal output.

- App-wide state is only partially fixed.
  - `default.vue` bootstraps settings, bot, strategies, and exchanges, but `app.vue` still calls `app.loadSettings()` separately.
  - Dashboard recent activity relies on `botStore.recentLogs`, but `botStore.init()` does not load historical logs into that store.
  - Charts and journal still do not appear to refresh directly from bot trade/equity events.
  - This does not fully satisfy the "same runtime state everywhere" requirement.

- Real exchange work is incomplete and unsafe to trust.
  - Authenticated balances appear only partially implemented for Binance and Bybit.
  - Other providers either fail or fall back to public reachability checks, which is not credential validation.
  - DEX providers remain unsupported in the backend despite being in the provider type/UI surface.
  - Static application encryption key remains. No OS keyring or user-derived secret was implemented.
  - No exchange order routing exists, so the system is not a real trading bot yet.

- Strategy developer experience was mostly skipped.
  - No real "Validate Strategy" action is exposed in the editor.
  - No params editor was added.
  - Save-before-deploy is not automatic and the dirty warning is not tied to actual editor changes.
  - Python import/runtime errors are not shown inline in the strategy editor.

- The implementation has product-quality gaps.
  - The deploy modal is a useful start, but the behavior underneath is too thin.
  - Several features are labels or warnings rather than enforced constraints.
  - The work improves the UI path, but it does not make the trading system reliable.

## Required Follow-Up Work

### P0 - Block Incorrect Trading Behavior

- Hard-block `trading_mode = "live"` in `start_bot` until real exchange order routing and safety checks exist.
- Add a backend guard that returns an error for live mode, not just a UI-disabled button.
- Rename backend runtime/event wording from `live` to `paper` where it is actually synthetic paper trading.
- Ensure `equity_snapshots.source` does not call synthetic paper data `live`.

### P0 - Fix Strategy Pair And State Sync

- Send selected pair to Python on start/configure and set `strategy._pair`.
- On `on_candle`, also set/update pair context from the candle if needed.
- Update Python runner/SDK state after fills:
  - maintain `strategy._positions`,
  - maintain `strategy._balance`,
  - remove/close positions when host fills a close,
  - expose current mark price if strategies depend on it.
- Add a manual test: deploy `ETH/USDT`; confirm the emitted order and persisted trade are `ETH/USDT`, not `BTC/USDT`.

### P0 - Replace Fake Dirty State

- Add actual dirty tracking in `strategies` store:
  - last loaded/saved editor content,
  - computed dirty flag,
  - reset after successful save/select.
- Pass the real dirty flag into every deploy entry point.
- Block Start or require explicit save/continue when dirty.
- Make `Save First` await save success and update modal state.

### P0 - Make Preflight Real

- Replace the lazy module import check with a real runner validation mode that:
  - imports the strategy file,
  - instantiates the strategy,
  - verifies exactly one usable `Strategy` subclass or a deterministic selection,
  - returns stderr/stdout traceback details.
- Make broken strategy import a fatal preflight error.
- Validate params JSON, timeframe, initial balance, risk per trade, max positions, slippage, exchange existence, pair support, and mode.
- For live mode, require credentials, supported provider, sandbox/testnet where available, market metadata, balance, min-size, precision, and explicit user confirmation.
- Log preflight results into the bot/preflight log stream.

### P0 - Harden Bot Lifecycle

- Do not mark `bot_state` as running until the Python child has successfully loaded and acknowledged startup.
- Add a startup handshake/ack from Python runner.
- Emit `bot:error` for spawn failure, strategy load failure, runtime exception, stdout/stderr reader failure, and unexpected process exit.
- Gracefully send `stop` and wait before killing the child process.
- Record crash/stale-state information so the user sees why the bot stopped.

### P1 - Fix Logs And Observability

- Persist logs to disk or SQLite with session ids.
- Make `get_bot_logs(limit, offset)` return newest logs by default or add an explicit sort direction.
- Load historical logs into `botStore.recentLogs`, not only the terminal component.
- Wire terminal log filters to the terminal output or remove the fake filters.
- Show deploy/preflight/start events in the terminal timeline.

### P1 - Complete Paper Trading Quality

- Replace the one-off synthetic loop with a named paper data provider interface.
- Add modes for synthetic, historical replay, and public exchange market data.
- Make commission and slippage separate; do not use the slippage field as fee rate.
- Add deterministic seed/session metadata so paper runs are reproducible.
- Add a session summary: realized PnL, unrealized PnL, drawdown, win rate, trade count, start/end equity.

### P1 - Refresh App-Wide State

- Remove duplicate settings load from `app.vue` or centralize bootstrap.
- Update charts from `bot:equity` and trades/journal from `bot:trade`, or add explicit refresh hooks.
- Ensure dashboard, titlebar, terminal, charts, and journal agree after app navigation and after reload.

### P1 - Clean Up Exchange Boundaries

- Clearly separate public market-data support from authenticated trading support.
- Do not call public ping "authenticated" or "connected" for unsupported providers.
- Either remove DEX providers from deploy/live surfaces or implement their wallet/rpc checks properly.
- Keep real trading provider scope small until one provider is actually correct end to end.

### P2 - Add Missing Developer UX

- Add a visible "Validate Strategy" action in the strategy toolbar/editor.
- Add a params editor with validation and save.
- Show Python validation errors inline with traceback summary.
- Add strategy template/docs for expected `Strategy` APIs.

## Acceptance Criteria For The Follow-Up

- Deploying `ETH/USDT` causes Python strategy orders, Rust runtime trades, dashboard positions, and journal rows to all use `ETH/USDT`.
- A strategy with a syntax/import error fails preflight and cannot start.
- A dirty strategy cannot be started without a save or explicit override.
- Directly invoking `start_bot` with `trading_mode = "live"` returns a hard error until real trading is implemented.
- A Python child crash emits `bot:error`, updates UI status, and leaves a visible error in terminal/history.
- Terminal shows latest session logs after navigation, not only logs emitted while terminal is mounted.
- Paper bot start/stop works with visible state consistency across titlebar, dashboard, terminal, charts, and journal.
- No UI claims real trading is available until authenticated balances, pair metadata, order placement, reconciliation, and safety gates are implemented for at least one provider.
