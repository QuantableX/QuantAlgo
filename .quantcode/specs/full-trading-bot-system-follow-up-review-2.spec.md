---
title: Full Trading Bot System - Follow Up Review (2)
status: open
priority: critical
linkedFiles:
  - .quantcode/specs/full-trading-bot-system.spec.md
  - .quantcode/specs/full-trading-bot-system-follow-up.spec.md
  - app/components/Deploy/Modal.vue
  - app/components/Strategy/StrategyToolbar.vue
  - app/components/Terminal/BotTerminal.vue
  - app/components/Layout/RightSidebar.vue
  - app/pages/charts.vue
  - app/pages/journal.vue
  - app/pages/index.vue
  - app/pages/settings.vue
  - app/pages/strategies.vue
  - app/stores/app.ts
  - app/stores/bot.ts
  - app/stores/exchange.ts
  - app/stores/journal.ts
  - app/stores/strategies.ts
  - app/types/index.ts
  - src-tauri/src/lib.rs
  - python/quantalgo/runner.py
  - python/quantalgo/strategy.py
  - python/quantalgo/models.py
createdAt: 2026-04-14T00:00:00.000Z
updatedAt: 2026-04-14T00:00:00.000Z
---

## Review Verdict

- Verdict: improved, but still not done. This is no longer the same failed attempt as review 1, but it still should not be treated as "fully functional" or "refined to perfection".
- The second agent fixed several of the big blockers from the first follow-up:
  - real dirty tracking was added in the strategies store,
  - live mode is now hard-blocked in preflight and `start_bot`,
  - pair/balance/position/mark-price state is now sent into the Python runner,
  - the Python runner has a real `--validate` mode,
  - bot logs are persisted and loaded back into memory,
  - charts and journal now listen to bot trade/equity events,
  - the selected deploy pair now mostly reaches the runtime.
- That said, this is still a paper trading system with real correctness holes. The most serious remaining issues are unit mistakes, duplicated validation, position-state collapse, misleading exchange support, missing strategy params UX, and insufficient safety/QA proof.
- Review method: static code review only. No builds, tests, app launches, or live exchange calls were run for this review.

## What Was Fixed Since Follow-Up Review 1

- Dirty state is no longer completely fake.
  - `app/stores/strategies.ts` now tracks `savedEditorContent` and exposes `isDirty`.
  - `app/pages/strategies.vue` and `app/pages/index.vue` now pass that dirty state into the deploy modal.
  - This is a real improvement, not just a label change.

- Live mode is now blocked in the backend.
  - `src-tauri/src/lib.rs:2917` blocks live mode during deploy preflight.
  - `src-tauri/src/lib.rs:2971` blocks direct `start_bot` calls with `trading_mode = "live"`.
  - That is the right call until real order routing, reconciliation, precision, min-size checks, and kill switches exist.

- The Python runner is materially better.
  - `python/quantalgo/runner.py:48` loads a deterministic `Strategy` subclass.
  - `python/quantalgo/runner.py:300` has a validation path.
  - `python/quantalgo/runner.py:113` and `python/quantalgo/runner.py:139` apply host state snapshots and trade updates.
  - This addresses the worst "deployed pair is ignored" and "strategy state is stale" problems from review 1, but not completely.

- Bot startup is safer.
  - `src-tauri/src/lib.rs:3154` sends a startup request to the child strategy.
  - `src-tauri/src/lib.rs:3169` waits for a startup acknowledgement before marking the bot running.
  - This is much better than marking running immediately after spawn.

- Logs are more useful.
  - `src-tauri/src/lib.rs:827` persists bot log entries.
  - `src-tauri/src/lib.rs:4899` loads persisted logs into memory on app startup.
  - `app/stores/bot.ts:187` loads logs into the bot store during init.
  - `app/components/Terminal/BotTerminal.vue:150` loads historical terminal logs.

## P0 Remaining Problems

- Strategy params are applied too late for `on_start`.
  - Rust sends `"method": "start"` at `src-tauri/src/lib.rs:3154`, waits for ack at `src-tauri/src/lib.rs:3169`, and only then sends `"method": "configure"` at `src-tauri/src/lib.rs:3190`.
  - Python calls `strategy.on_start()` before `configure` in `python/quantalgo/runner.py:241`.
  - Any strategy that reads `self.params` in `on_start()` will run with default params, not saved UI params. This is a real behavior bug.
  - Fix: send params in the initial start payload or send configure before start and require an ack for both.

- Fee and slippage are still incorrectly mixed.
  - `src-tauri/src/lib.rs:3058` derives `fee_rate` from the `slippage` config.
  - `src-tauri/src/lib.rs:3063` also derives `slippage_pct` from the same field.
  - That means one UI setting is being used both as exchange fee and execution slippage. PnL, equity, and trade stats will be wrong.
  - Fix: add separate paper commission/fee config, keep slippage as slippage, and make the labels match the math.

- Risk/slippage units are inconsistent across frontend, backend defaults, paper trading, and backtesting.
  - Frontend defaults in `app/stores/app.ts:17` use `risk_per_trade = 1` and `slippage_tolerance = 0.1`.
  - Rust defaults in `src-tauri/src/lib.rs:321` use `risk_per_trade = 0.02` and `slippage_tolerance = 0.001`.
  - Settings labels show percent values in `app/pages/settings.vue:190` and `app/pages/settings.vue:215`.
  - Backtest multiplies `settings.slippage_tolerance * 100.0` at `src-tauri/src/lib.rs:1609`, while paper trading uses the raw deploy slippage as a percent at `src-tauri/src/lib.rs:3063` and `src-tauri/src/lib.rs:2151`.
  - Result: a user setting that means 0.1 percent in the UI can become 10 percent slippage in backtests but 0.1 percent in paper trading. That makes backtest vs paper results untrustworthy.
  - Fix: choose one canonical unit, probably percentage points in UI/backend config, remove the backtest `* 100.0`, migrate existing settings if needed, and add explicit tests.

- Position synchronization still collapses multiple same-pair positions.
  - Rust stores open positions in a `Vec<LivePosition>` and allows `max_positions > 1`.
  - `runtime_positions_json` inserts positions into a JSON map keyed only by `position.pair` at `src-tauri/src/lib.rs:914`.
  - Python stores positions in `_positions: Dict[str, Position]` keyed by pair at `python/quantalgo/strategy.py:24`.
  - `Position` has no id in `python/quantalgo/models.py:116`.
  - If the runtime opens multiple BTC/USDT positions, Python only sees the last one for that pair. Closing by `position_id` cannot be represented cleanly in Python state.
  - Fix: either enforce one open position per pair/side everywhere, or change Python positions to be keyed by position id and expose helpers for pair lookup.

- Preflight still has duplicated and misleading strategy validation.
  - The old lazy import check remains at `src-tauri/src/lib.rs:2691`.
  - The newer real runner validation starts at `src-tauri/src/lib.rs:2720`.
  - The lazy check can say "Strategy module loads without errors" while runner validation fails to instantiate a real `Strategy`. That creates contradictory checklist output.
  - Fix: delete the lazy import check and keep only the runner validation path with traceback details.

- Backend risk limits are not serious safety limits.
  - `risk_per_trade <= 100`, `max_positions <= 100`, and `slippage <= 50` are accepted as OK in `src-tauri/src/lib.rs:2515`, `src-tauri/src/lib.rs:2533`, and `src-tauri/src/lib.rs:2551`.
  - The settings UI caps max positions at 50 and slippage at 10, but direct backend calls still accept much worse values.
  - Calling 100 percent risk and 50 percent slippage "ok" is not a refined trading bot. It is a foot-gun.
  - Fix: add realistic hard caps and warning bands, and make the backend stricter than or equal to the UI.

- Paper mode depends on public exchange pair metadata every time it starts.
  - `start_bot` calls `fetch_exchange_pairs_for_provider` at `src-tauri/src/lib.rs:3013`.
  - Pair discovery makes blocking public HTTP calls with 15 second timeouts in `src-tauri/src/lib.rs:4056`.
  - This is fine for real exchange validation, but it makes synthetic paper trading fail when public APIs are down, rate-limited, geo-blocked, or offline.
  - Fix: separate "synthetic paper provider" from "public exchange metadata validation". Add caching, refresh timestamps, and a clear offline/synthetic mode if paper is meant to work without exchange network access.

- Strategy runtime errors are over-classified as fatal bot errors.
  - The stderr reader turns any line containing `"ERROR"` into `emit_bot_error` at `src-tauri/src/lib.rs:3133`.
  - The Python runner logs handler exceptions with `log.error` and then continues the JSON-RPC loop at `python/quantalgo/runner.py:205`.
  - A recoverable strategy exception can flip the whole UI to `error` while the child process is still alive.
  - Fix: distinguish fatal startup failure, fatal process exit, and recoverable strategy event errors. Emit different event types or include a severity.

## P1 Product And UX Gaps

- There is still no visible "Validate Strategy" action.
  - `app/components/Strategy/StrategyToolbar.vue:84` exposes New/Delete/Save/Run Backtest/Deploy, but no Validate action.
  - Runner validation exists now, but it is buried inside deploy preflight.
  - Fix: add a toolbar/editor validation button and show Python traceback summaries inline.

- There is still no strategy params editor.
  - Saved `params_json` is validated during preflight, but the user still has no proper UI to edit it.
  - The code re-saves existing params in `app/pages/strategies.vue:27` and `app/pages/index.vue:107`, but this is not a product-quality params workflow.
  - Fix: add a params editor with JSON validation, save errors, and deploy preview.

- Save-before-deploy failures are still mostly console-only.
  - `app/pages/strategies.vue:33` catches save errors and logs to console.
  - `app/pages/index.vue:113` does the same for dashboard save-before-deploy.
  - The deploy modal only sees that the strategy remains dirty; it does not show the save failure reason.
  - Fix: make save-first return success/failure to the modal and render the error beside the dirty warning.

- Deploy preflight can become stale and noisy.
  - `deployConfig` includes settings-derived risk/slippage at `app/components/Deploy/Modal.vue:91`.
  - The auto-run watcher at `app/components/Deploy/Modal.vue:151` does not watch risk settings, max positions, or slippage.
  - It also runs preflight on every selected field change, and backend preflight may do public exchange HTTP calls.
  - Fix: watch the full config, debounce preflight, cancel or ignore stale requests, and cache pair metadata.

- Right-sidebar chart controls are dead UI.
  - `app/components/Layout/RightSidebar.vue:45` has its own `selectedTimeframe`.
  - `app/pages/charts.vue:7` has a separate `selectedTimeframe`.
  - Clicking the sidebar timeframe buttons at `app/components/Layout/RightSidebar.vue:261` does not change the actual chart page.
  - Fix: either remove those controls or wire them through a shared chart store/query param.

- Journal stats are calculated from paginated rows.
  - `app/stores/journal.ts:11` defaults filters to `limit: 50`.
  - `src-tauri/src/lib.rs:4296` computes stats by calling `list_trades(filters, state)`, so `limit` and `offset` affect the stats.
  - This makes win rate, profit factor, best/worst trade, and total PnL describe the current page, not the full filtered journal.
  - Fix: stats queries must ignore pagination while keeping the other filters.

- Terminal polish is still rough.
  - `app/components/Terminal/BotTerminal.vue:152` contains mojibake in the historical log header.
  - `app/components/Terminal/BotTerminal.vue:160` labels the stream as "Live" even though the only enabled trading mode is paper.
  - Fix: replace corrupted strings and use "New logs" or "Current session" instead of "Live".

- Exchange support is still over-presented in the UI.
  - `app/components/Exchange/ExchangeForm.vue:15` exposes only five CEX providers, but `src-tauri/src/lib.rs:3903` still has a public-ping fallback for unsupported providers.
  - `app/components/Exchange/ExchangeForm.vue:16` exposes DEX providers, while `fetch_exchange_pairs_for_provider` returns unsupported-provider errors at `src-tauri/src/lib.rs:4148`.
  - Fix: clearly label unsupported DEX providers as "stored credentials only / deploy unsupported", or hide them from deploy until pair discovery and routing exist.

- Exchange credentials are still protected by a static application key.
  - `src-tauri/src/lib.rs:330` hard-codes `APP_ENCRYPTION_KEY`.
  - This is better than plaintext but still not a serious secrets model.
  - Fix: use OS keyring or a user/device-derived key and document migration for existing encrypted configs.

- Paper execution still lacks a defined margin/short model.
  - Long opens deduct notional plus fee, but short opens only deduct fee at `src-tauri/src/lib.rs:2222`.
  - Short closes add PnL at `src-tauri/src/lib.rs:2323`.
  - That may be acceptable for a toy margin model, but it is not documented or risk-limited.
  - Fix: define the paper account model: spot-only, margin, or futures. Then enforce collateral, leverage, liquidation assumptions, and allowable order sides.

- Logs persist to one growing flat file without session boundaries.
  - `src-tauri/src/lib.rs:836` appends to `bot.log`.
  - There is no session id, rotation policy, or deploy/run id attached to log entries.
  - Fix: persist logs by session/run id in SQLite or rotate structured log files with a clear latest-session query.

## P2 Hygiene And Maintainability

- Generated Python bytecode is still tracked and modified.
  - `git ls-files python/quantalgo/__pycache__` shows tracked `.pyc` files.
  - Current diff includes modified `.pyc` files under `python/quantalgo/__pycache__`.
  - Fix: remove tracked bytecode from the repo and add an ignore rule for `__pycache__/` and `*.pyc`.

- `src-tauri/src/lib.rs` is becoming a monolith.
  - The current diff adds thousands of lines to one Rust file.
  - Bot runtime, exchange adapters, persistence, import/export, preflight, backtest, and app commands are all tangled together.
  - Fix after behavior is stable: split into modules such as `bot`, `preflight`, `exchange`, `journal`, `backtest`, and `settings`.

- The Tauri resize listener is not cleaned up.
  - `app/layouts/default.vue:95` registers `appWindow.onResized(...)`.
  - `onUnmounted` removes the DOM keydown listener but does not unlisten the Tauri resize handler.
  - Fix: store the unlisten function and call it on unmount.

## Required Follow-Up Work

### P0 - Correct Trading Math And Runtime State

- Split fee and slippage into separate config fields.
- Normalize risk and slippage units across settings, deploy, paper trading, and backtest.
- Fix strategy params ordering so params are available before `on_start`.
- Fix Python position state so multiple same-pair positions cannot silently collapse.
- Decide whether the paper engine supports one position per pair, spot-only trading, or margin/short trading. Enforce that decision in code.

### P0 - Make Preflight Trustworthy

- Remove the old lazy import check.
- Keep only runner validation and make failures fatal.
- Add stricter backend caps for risk, slippage, and max positions.
- Add stale-request protection/debounce for frontend preflight.
- Cache public pair metadata and define how synthetic paper works when exchange metadata is unavailable.

### P1 - Finish The Missing User Workflows

- Add a visible Validate Strategy button.
- Add a real strategy params editor.
- Return save-first errors into the deploy modal instead of console-only logging.
- Wire sidebar chart controls or remove them.
- Fix journal stats so they are not limited to the current page.
- Fix terminal labels and mojibake.

### P1 - Exchange And Security Boundaries

- Keep live trading disabled until a single provider works end to end.
- Hide or clearly label unsupported DEX deploy paths.
- Remove public-ping fallback from any flow that claims authenticated exchange support.
- Replace the static encryption key with keyring/user-derived secrets.

### P2 - Repo Hygiene

- Stop tracking `.pyc` files.
- Add ignore rules for generated Python bytecode.
- Split `src-tauri/src/lib.rs` into focused modules after the behavioral fixes land.
- Add a visible QA checklist and test evidence before claiming completion.

## Acceptance Criteria For Review 2

- Strategy params are available inside `on_start()` for a deployed strategy.
- A 0.1 percent slippage setting means 0.1 percent in settings, deploy, paper fills, and backtests.
- Fees and slippage are separate in config, fills, persisted trades, and UI labels.
- Opening multiple positions on the same pair either works correctly in Python state or is explicitly blocked by the runtime.
- Preflight shows one strategy validation result, not both lazy import and runner validation.
- Backend rejects reckless direct values such as 100 percent risk per trade and 50 percent slippage.
- Synthetic paper mode can be started with a deliberate synthetic provider path, or the UI clearly explains that public exchange metadata is required.
- Save-before-deploy failures are visible in the deploy modal.
- Journal stats ignore pagination.
- Right-sidebar chart controls either affect the charts page or are removed.
- Terminal headings are not mojibake and do not imply live trading.
- `.pyc` files are no longer tracked or modified in normal development.
- A final pass includes actual command/test evidence, not only static review.
