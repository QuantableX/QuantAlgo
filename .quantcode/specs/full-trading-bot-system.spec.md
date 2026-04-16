---
title: Full Trading Bot System
status: open
priority: critical
linkedFiles:
  - .quantcode/specs/full-trading-bot-system.spec.md
  - app/pages/strategies.vue
  - app/stores/bot.ts
  - app/stores/exchange.ts
  - app/layouts/default.vue
  - app/components/Strategy/StrategyToolbar.vue
  - app/components/Terminal/BotTerminal.vue
  - src-tauri/src/lib.rs
  - python/quantalgo/runner.py
  - python/quantalgo/strategy.py
createdAt: 2026-04-13T21:12:54.489Z
updatedAt: 2026-04-13T21:12:54.489Z
---


## Requirements

- Make the trading bot feel like a complete product, not a demo: users can choose a strategy, exchange, pair, mode, risk settings, and start/stop the bot with clear status and recoverable errors.
- Fix the deploy/start flow so it is explicit, validated, and user-visible. The current Deploy action must not silently fail or hard-code the first exchange and BTC/USDT.
- Separate paper trading from real trading. Paper mode can use simulated candles and balances, but real mode must only be enabled after exchange credentials, pair support, balances, order routing, and safety checks are implemented.
- Provide a deployment preflight checklist before start: saved strategy, valid Python runtime, importable strategy class, valid params, selected exchange, selected pair, available balance, risk limits, and chosen trading mode.
- Make runtime state reliable across the app: titlebar, dashboard, terminal, charts, journal, and strategy screen should all reflect the same bot status, active strategy, active exchange, pair, logs, equity, and positions.
- Replace console-only failures with visible UI errors, toasts/panels, and actionable messages.
- Keep logs and terminal useful: show startup steps, preflight results, Python load errors, strategy exceptions, market data events, order decisions, fills, stops, and crashes.
- Refine the UI so deployment is a guided flow instead of a single ambiguous toolbar button.
- Add acceptance tests/manual QA for start, stop, deploy failure, Python missing, no exchange, invalid strategy, paper trade open/close, app reload while running, and saved logs/trades.

## Current State

- The QuantCode spec was empty before this plan.
- Frontend is Nuxt 3 + Pinia + Tauri invokes. Main deployment entry is `app/pages/strategies.vue`.
- `handleDeploy()` currently:
  - requires only an active strategy,
  - takes `exchangeStore.exchanges[0]`,
  - hard-codes `BTC/USDT`,
  - does not save dirty editor content before deploy,
  - does not validate Python, params, pair support, balances, or bot status,
  - logs most failures only to the browser console.
- `app/stores/bot.ts` can call `start_bot`, `stop_bot`, `get_bot_status`, and listen for Tauri events, but app-wide initialization is inconsistent. Some global UI can show stale status until a page initializes the store.
- `src-tauri/src/lib.rs` has a working Tauri command surface and `cargo check` passed, but the current "live" bot path is actually a synthetic paper loop:
  - starts a Python child process,
  - sends generated candles every second,
  - simulates fills and balances in Rust,
  - persists trades/equity snapshots,
  - does not execute real exchange orders.
- Exchange integration is incomplete:
  - `test_exchange_connection` only pings public endpoints,
  - `get_balances` returns placeholder balances,
  - `get_exchange_pairs` returns a static list,
  - credential storage exists, but real private API signing/order placement is not complete.
- Python SDK/runner exists and supports strategy loading plus JSON-line RPC, but the host does not fully keep Python strategy state in sync with live positions/balances after simulated fills.
- Terminal listens to `bot:log`, but historical logs are not loaded into the terminal on page entry, so important startup errors can be missed.
- UI has several clunky product gaps: no deploy modal, no exchange/pair selector during deploy, no risk summary, no start mode label, no preflight output, no persistent deployment config, no visible deploy failure state.

## Missing For "Fully Functional"

- Deployment wizard/modal:
  - strategy selector and dirty-save warning,
  - exchange selector,
  - pair selector loaded from the selected exchange,
  - paper/live mode selector,
  - timeframe and initial balance for paper mode,
  - risk per trade, max positions, slippage, and emergency stop summary,
  - final preflight checklist and explicit Start button.
- Backend preflight command:
  - add a `validate_bot_deploy` or equivalent Tauri command,
  - verify strategy exists and file is readable,
  - validate params JSON,
  - run Python detection/load check before starting,
  - verify selected exchange exists and pair is supported,
  - verify mode-specific requirements,
  - return structured warnings/errors for the UI.
- Bot lifecycle hardening:
  - reject double-start with a visible UI message,
  - record startup failures back to `bot_state`,
  - emit `bot:error` on spawn/load/runtime errors,
  - stop market thread and Python process cleanly,
  - recover stale `running` DB state on app startup if the child process is gone,
  - include `exchange_id`, `pair`, `started_at`, and `config_json` in status events, not only `status` and `strategy_id`.
- Paper trading engine refinement:
  - label the current simulated engine clearly as Paper,
  - use selected pair/timeframe/risk config,
  - keep strategy balances and positions synchronized after fills,
  - make fill logic deterministic enough for debugging,
  - expose open positions and current mark price accurately in dashboard/charts.
- Real exchange execution:
  - implement authenticated balance fetch per supported CEX,
  - implement public pair/market metadata fetch per provider,
  - implement order placement/cancel/close with exchange-specific precision, min-size, fees, and response parsing,
  - add dry-run/sandbox support before enabling real funds,
  - add order id mapping and reconciliation loop,
  - persist fills and execution errors.
- Strategy developer experience:
  - add "Validate Strategy" action,
  - show Python import/runtime errors inline,
  - expose and edit `params_json` in the UI,
  - save before backtest/deploy or prompt when editor content is dirty,
  - provide strategy template docs/snippets.
- Observability:
  - load existing logs when Terminal opens,
  - add log filters that actually filter terminal output,
  - show deploy preflight logs,
  - keep a bounded persisted log file/session history,
  - show a clear bot error banner with recovery action.
- App-wide state:
  - initialize app settings, bot status, strategies, and exchange list from a single app bootstrap path,
  - make titlebar/dashboard/right sidebar rely on hydrated stores,
  - refresh chart/journal data after live/paper trade events,
  - handle app reload while a bot is running.
- Security and safety:
  - replace static encryption key with OS keyring or user-derived secret,
  - never show secrets after save,
  - require explicit confirmation for real trading,
  - add max loss / kill switch / max position exposure checks,
  - default to paper mode until real trading is fully implemented and tested.

## Implementation Plan

### Phase 1 - Make deploy/start usable in paper mode

- Replace `handleDeploy()` with a Deploy modal/wizard.
- Load exchanges and pairs before deploy; require a selected exchange and pair.
- Use settings defaults for pair/timeframe/risk, but allow overrides.
- Save or prompt for unsaved strategy code before starting.
- Add visible loading, success, and error states instead of console-only deploy errors.
- Update `bot:status` event payload to include full `BotStatus` fields and update `app/stores/bot.ts` accordingly.
- Initialize `botStore.init()`, `strategiesStore.load()`, and `exchangeStore.load()` at app/layout startup so the titlebar and right sidebar are never stale.
- Label the current mode as `paper` everywhere it appears.

### Phase 2 - Add preflight validation

- Add a Rust preflight command that returns structured checks with `id`, `label`, `status`, `severity`, and `message`.
- Validate Python availability using the configured interpreter.
- Validate strategy file existence and importability by launching the runner in a check mode or adding a lightweight runner validation command.
- Validate selected exchange and pair.
- Validate initial balance/risk/max positions/timeframe.
- Show preflight results in the deploy modal and block Start on fatal errors.

### Phase 3 - Harden bot lifecycle and runtime state

- Record startup failures in `bot_state` and emit `bot:error`.
- Cleanly stop Python and market threads; avoid only killing the process when graceful stop is possible.
- Reconcile DB `bot_state` on app startup.
- Persist and restore recent logs into the Terminal.
- Keep Python strategy `_positions` and `_balance` synchronized after simulated fills.
- Ensure dashboard open positions use current mark price, not `exit_price ?? entry_price`.

### Phase 4 - Make paper trading feel complete

- Replace hard-coded synthetic behavior with a paper data provider interface:
  - synthetic candles,
  - historical replay,
  - exchange public market data where available.
- Add paper execution settings for commission, slippage, latency, fill policy, and initial capital.
- Add position sizing/risk enforcement from settings.
- Add a paper trading session summary with realized PnL, unrealized PnL, drawdown, win rate, and trade count.

### Phase 5 - Real exchange integration

- Implement provider adapters behind a common exchange trait/interface.
- Start with one CEX provider before broadening scope, preferably Binance spot or Coinbase, because pair metadata, balances, and order APIs are straightforward compared with DEX wallets.
- Implement authenticated balance fetch, pair metadata, market order placement, cancel, close, and reconciliation.
- Add sandbox/testnet mode where supported.
- Gate real trading behind explicit confirmation and safety checks.
- Keep paper mode as the default.

### Phase 6 - Polish and QA

- Add manual QA scripts/checklists:
  - no strategy selected,
  - unsaved strategy,
  - invalid Python path,
  - strategy import error,
  - no exchange configured,
  - unsupported pair,
  - start paper bot,
  - stop bot,
  - process crash,
  - reload app while bot is running,
  - terminal history,
  - charts/journal update after trades.
- Add automated coverage where practical:
  - Rust unit tests for preflight and state transitions,
  - Python runner tests for strategy loading and dispatch,
  - frontend component/store tests for deploy modal validation if the project adds a test runner.
- Review copy and UI labels so users understand when they are in paper mode vs real trading.

## Definition Of Done

- A user can start a paper bot without editing code or relying on hard-coded exchange/pair defaults.
- Deploy/start failures are visible in the UI and explain the required fix.
- Bot status is consistent in the titlebar, dashboard, strategy page, terminal, charts, and journal.
- Terminal shows startup/preflight/runtime logs, including logs emitted before the Terminal page opens.
- Paper trades update positions, equity, charts, and journal reliably.
- Real trading is either fully implemented behind explicit safety gates or clearly unavailable/disabled in the UI.
- The app has a documented QA checklist for the full start/stop/deploy path.
