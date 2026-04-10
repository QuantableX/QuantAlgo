# QuantAlgo — Trading Bot Terminal

## Overview

QuantAlgo is a desktop trading bot terminal for crypto markets (CEX & DEX). It provides a strategy editor for writing Python trading algorithms, a multi-strategy backtester with saveable results, a live bot activity log/terminal, a stats/journal view, equity curve visualization with active trade overlays, and exchange/wallet integration for live trading execution.

---

## Tech Stack

### Frontend
- **Framework:** Nuxt 3 (`nuxt ^3.15`, `ssr: false`, `srcDir: 'app/'`)
- **UI:** Vue 3 Composition API (`<script setup lang="ts">`)
- **State:** Pinia (`@pinia/nuxt`)
- **Utilities:** VueUse (`@vueuse/nuxt`, `@vueuse/core`)
- **Styling:** Tailwind CSS v4 (`@tailwindcss/vite` plugin, no PostCSS config)
- **Code Editor:** Monaco Editor (`monaco-editor ^0.52`) — for Python strategy editing
- **Terminal:** xterm.js (`@xterm/xterm ^5.5`, `@xterm/addon-fit`, `@xterm/addon-webgl`, `@xterm/addon-unicode11`) — for bot activity log
- **Charts:** Lightweight Charts (`lightweight-charts`) — for equity curve and trade visualization
- **TypeScript:** strict, `vue-tsc` for type checking

### Desktop Shell
- **Runtime:** Tauri v2
- **Backend language:** Rust (edition 2021)
- **Plugins:** `tauri-plugin-shell` (spawn Python processes), `tauri-plugin-dialog` (file open/save)
- **Crate structure:** `src-tauri/` with `lib.rs` as `quantalgo_lib` (crate-type: `["lib", "cdylib", "staticlib"]`)
- **Key Rust deps:** `serde`, `serde_json`, `tokio` (full), `portable-pty` (terminal), `rusqlite` (bundled, local DB), `reqwest` (exchange API calls), `uuid`, `dirs`

### Python Runtime
- Strategies are `.py` files executed by a system-installed or bundled Python interpreter
- Communication between Tauri (Rust) and Python via stdin/stdout JSON-RPC over a spawned child process (using `portable-pty` or `tauri-plugin-shell`)
- Python process lifecycle: Rust spawns → sends config/strategy → receives trade signals, logs, backtest results → can kill/restart

---

## Design System

Matches the Quant suite (QuantCode, QuantMCP). Dark-first, monochrome palette with surgical accent color.

### Color Tokens (CSS custom properties on `:root`)

```css
:root {
  --qa-bg:             #18181e;   /* App background */
  --qa-bg-sidebar:     #1f1f25;   /* Sidebar / secondary bg */
  --qa-bg-card:        #292930;   /* Cards, panels, header bars */
  --qa-bg-hover:       #313139;   /* Hover states */
  --qa-bg-input:       #18181e;   /* Input fields */
  --qa-border:         #47474f;   /* Borders */
  --qa-border-subtle:  #4a4a55;   /* Subtle dividers */
  --qa-text:           #d4d4d8;   /* Primary text */
  --qa-text-secondary: #9a9aa5;   /* Secondary text */
  --qa-text-muted:     #6e6e7a;   /* Muted/dim text */
  --qa-accent:         #a0a0a8;   /* Accent (monochrome) */
  --qa-accent-hover:   #b8b8c0;   /* Accent hover */
  --qa-success:        #2ed573;   /* Profit / long / running */
  --qa-error:          #ff4757;   /* Loss / short / error */
  --qa-warning:        #ffa502;   /* Warning / pending */
  --qa-scrollbar-track: #18181e;
  --qa-scrollbar-thumb: #47474f;
  --qa-scrollbar-thumb-hover: #5a5a64;
  --qa-radius:         8px;
  --qa-radius-lg:      12px;
  --qa-transition:     150ms ease;
}
```

### Light Theme
Toggle via `[data-theme="light"]` on `<html>`. Invert luminance following QuantCode/QuantMCP pattern:
```css
[data-theme="light"] {
  --qa-bg:             #d8d8de;
  --qa-bg-sidebar:     #e0e0e5;
  --qa-bg-card:        #e6e6ec;
  --qa-bg-hover:       #c8c8d0;
  --qa-bg-input:       #d8d8de;
  --qa-border:         #a5a5af;
  --qa-text:           #24242c;
  --qa-text-secondary: #484856;
  --qa-text-muted:     #6a6a78;
  --qa-accent:         #4a4a52;
  --qa-accent-hover:   #3a3a42;
}
```

### Typography
- **Sans:** `-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif`
- **Mono:** `ui-monospace, 'Cascadia Code', 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace`
- Base font size: 14px, line-height: 1.5
- `-webkit-font-smoothing: antialiased`

### Scrollbars
6px width, rounded 3px thumb, matching `--qa-scrollbar-*` tokens. Identical to QuantCode/QuantMCP.

### Icons
- App icon: `icons/icon.png` (the Q target logo)
- Text logo: `icons/QuantAlgo.png` — used in sidebar header
- All platform variants already exist in `icons/`

---

## Application Layout

Three-column layout matching QuantMCP, adapted for trading context:

```
┌──────────────────────────────────────────────────────────────────────┐
│ [Sidebar 220px] │ [Main Content area]              │ [Right 260px]  │
│                 │                                   │                │
│  Logo/Brand     │  ┌─ Top Status Bar ─────────────┐ │  Theme Toggle  │
│                 │  │ Bot: ● Running  │ PnL +2.3%  │ │  + Settings    │
│  ── Nav ──      │  └──────────────────────────────┘ │                │
│  Strategies     │                                   │  Context panel │
│  Backtest       │  [Page content rendered here]     │  (changes per  │
│  Terminal       │                                   │   active page) │
│  Journal        │                                   │                │
│  Charts         │                                   │                │
│  Exchange       │                                   │                │
│  Settings       │                                   │                │
│                 │                                   │                │
└──────────────────────────────────────────────────────────────────────┘
```

### Left Sidebar (220px, `--qa-bg-sidebar`)
- **Header:** QuantAlgo text logo (`icons/QuantAlgo.png`, height 22px), links to dashboard
- **Navigation items** with Unicode icons + label, 14px, 500 weight:
  - `⊞` **Dashboard** — `/` — overview of all bots, quick stats
  - `✎` **Strategies** — `/strategies` — list/create/edit Python strategies
  - `⧉` **Backtest** — `/backtest` — run and compare backtests
  - `▤` **Terminal** — `/terminal` — live bot activity log (xterm)
  - `☰` **Journal** — `/journal` — trade journal & stats
  - `◧` **Charts** — `/charts` — equity curve & active trades
  - `⚙` **Exchange** — `/exchange` — exchange/wallet connections
  - `⛭` **Settings** — `/settings` — app configuration
- Active state: `--qa-bg-hover` bg, `--qa-accent` text color
- Hover state: `--qa-bg-hover` bg

### Top Status Bar (51px min-height, `--qa-bg-sidebar`)
- **Left:** Status dot (green=running, gray=stopped, red=error) + "Bot Status" label + active strategy name chip
- **Right:** Metrics pills — `PnL: +2.3%`, `Open Trades: 3`, `Uptime: 4h 23m`
- Same pill/chip styling as QuantMCP (rounded-full, 11px uppercase, border)

### Right Sidebar (260px, `--qa-bg-sidebar`)
- **Header row:** Theme toggle pill (light/dark, identical to QuantMCP) + Settings gear icon
- **Context-sensitive content** below, depending on active route:
  - **Dashboard:** Quick account balances, connected exchanges list
  - **Strategies:** List of saved strategies, click to select/edit
  - **Backtest:** List of saved backtest results, click to load
  - **Terminal:** Filter controls (log level, strategy filter)
  - **Journal:** Date range filter, strategy filter
  - **Charts:** Timeframe selector, overlay toggles
  - **Exchange:** List of configured exchanges/wallets

### Main Content Area
- `flex: 1`, `overflow-y: auto`, `padding: 32px`
- Page content rendered via `<NuxtPage />`

---

## Pages & Features

### 1. Dashboard (`/`) — `pages/index.vue`

Overview grid of cards showing:
- **Bot status card:** Running/stopped indicator, current strategy, uptime, start/stop button
- **PnL summary card:** Today, 7d, 30d, all-time P&L with percentage and absolute values
- **Open positions card:** List of active trades (pair, side, entry, current, unrealized PnL)
- **Recent activity card:** Last 10 log entries from the bot (timestamp + message)
- **Quick actions:** Start bot, stop bot, switch strategy, open backtest

Card styling: `--qa-bg-card` bg, `--qa-border` 1px border, `--qa-radius-lg` corners, 16px padding.

### 2. Strategies (`/strategies`) — `pages/strategies.vue`

Split layout: strategy list (left/sidebar) + Monaco editor (right).

- **Strategy list (right sidebar context):**
  - Each strategy: name, description, created/modified date
  - New strategy button, duplicate, delete
  - Strategies stored as `.py` files in a user-configurable directory (default: `~/.quantalgo/strategies/`)

- **Monaco editor (main area):**
  - Full-height Python editor with syntax highlighting, autocomplete
  - Custom QuantAlgo Python SDK autocomplete (strategy base class methods)
  - Top toolbar: strategy name (editable), Save (Ctrl+S), Run Backtest, Deploy to Bot
  - Editor theme matching app theme (dark/light)

- **Strategy template:**
  ```python
  from quantalgo import Strategy, Order

  class MyStrategy(Strategy):
      """Strategy description here."""

      # Parameters (editable in UI)
      params = {
          "fast_period": 12,
          "slow_period": 26,
          "risk_per_trade": 0.02,
      }

      def on_candle(self, candle):
          """Called on each new candle."""
          pass

      def on_tick(self, tick):
          """Called on each price tick (live only)."""
          pass

      def on_trade(self, trade):
          """Called when a trade is filled."""
          pass

      def on_start(self):
          """Called when bot starts."""
          pass

      def on_stop(self):
          """Called when bot stops."""
          pass
  ```

### 3. Backtest (`/backtest`) — `pages/backtest.vue`

- **Configuration panel (top):**
  - Strategy selector (dropdown of saved strategies)
  - Exchange / trading pair selector
  - Date range picker (start/end)
  - Timeframe selector (1m, 5m, 15m, 1h, 4h, 1d)
  - Initial capital input
  - Commission/fee input
  - "Run Backtest" button

- **Results area (below config, after run):**
  - **Equity curve chart** (Lightweight Charts — area/line chart)
  - **Stats grid:** Total return, Sharpe ratio, max drawdown, win rate, profit factor, total trades, avg trade duration
  - **Trade list table:** Entry time, exit time, pair, side, entry price, exit price, PnL, PnL %
  - **Drawdown chart** (Lightweight Charts — area chart, inverted)

- **Multi-backtest comparison:**
  - Save backtest results with a name
  - Right sidebar shows saved results list
  - Select multiple to overlay equity curves on the same chart
  - Comparison table of stats side-by-side

- **Storage:** Backtest results saved as JSON in `~/.quantalgo/backtests/` via Rust/rusqlite

### 4. Terminal (`/terminal`) — `pages/terminal.vue`

Full-height xterm.js terminal displaying live bot output.

- **Features:**
  - Real-time streaming of bot stdout/stderr
  - Color-coded log levels: `INFO` (default), `TRADE` (green), `WARN` (yellow), `ERROR` (red)
  - Timestamp prefix on each line
  - Auto-scroll to bottom (toggleable)
  - Search within terminal output (Ctrl+F)
  - Clear terminal button
  - Export log to file

- **Right sidebar context:** Log level filter checkboxes, strategy filter dropdown

- **Implementation:** Rust spawns Python process via `portable-pty`, streams output to frontend via Tauri events (`tauri::Emitter`). Frontend renders in xterm.js instance.

### 5. Journal (`/journal`) — `pages/journal.vue`

Trade journal / stats dashboard.

- **Stats header row:** Total trades, win rate, avg win, avg loss, profit factor, expectancy, best/worst trade
- **Calendar heat map:** Daily PnL colored cells (green gradient for profit, red for loss)
- **Trade log table:** Sortable/filterable table of all historical trades
  - Columns: Date, Pair, Side (Long/Short), Entry, Exit, PnL ($), PnL (%), Duration, Strategy, Notes
  - Click a trade row to expand details / add notes
- **Filters (right sidebar):** Date range, strategy, pair, side (long/short), min PnL

- **Storage:** Trades stored in local SQLite via rusqlite (`~/.quantalgo/quantalgo.db`)

### 6. Charts (`/charts`) — `pages/charts.vue`

Equity curve and active trade visualization.

- **Main chart (Lightweight Charts):**
  - Equity curve as area/line series
  - Trade markers: green triangles (buy/long entry), red triangles (sell/short entry), exit markers
  - Hover tooltip showing trade details
  - Timeframe selector: 1h, 4h, 1d, 1w, 1M, All

- **Secondary chart (below):**
  - Drawdown visualization (area, red tint)
  - Or: cumulative PnL bar chart by day/week

- **Right sidebar:** Chart type toggles, overlay options (show/hide trades, show drawdown), timeframe buttons

### 7. Exchange (`/exchange`) — `pages/exchange.vue`

Exchange and wallet integration management.

- **Supported exchange types:**
  - **CEX:** Binance, Bybit, OKX, Coinbase, Kraken (via REST API + WebSocket)
  - **DEX:** Uniswap, Jupiter/Raydium (Solana), Hyperliquid (via RPC / SDK)

- **Connection management:**
  - Add exchange: Name, Type (CEX/DEX), API key, API secret, passphrase (if needed)
  - For DEX: Wallet address, private key (stored encrypted), RPC endpoint
  - Test connection button
  - Status indicator per exchange (connected/disconnected/error)

- **Account overview (per connected exchange):**
  - Balance display (total, available, in positions)
  - Asset breakdown list
  - Recent deposits/withdrawals

- **Security:**
  - API keys encrypted at rest using Tauri's secure storage or OS keychain
  - Private keys never leave the Rust backend
  - All exchange API calls happen in Rust, not frontend

- **Storage:** Exchange configs in `~/.quantalgo/exchanges.json` (encrypted sensitive fields)

### 8. Settings (`/settings`) — `pages/settings.vue`

- **General:** Theme (dark/light), font size, default trading pair, default timeframe
- **Python:** Python interpreter path (auto-detect or manual), pip packages management
- **Trading defaults:** Default risk per trade, max concurrent positions, slippage tolerance
- **Notifications:** Desktop notifications on trade fill, on error, on daily summary
- **Data:** Strategy directory path, backtest data directory, export/import all data
- **About:** App version (from `package.json`), links

---

## Data Architecture

### Local Database (`~/.quantalgo/quantalgo.db` — SQLite via rusqlite)

```sql
-- Trades (both backtest and live)
CREATE TABLE trades (
    id TEXT PRIMARY KEY,
    strategy_id TEXT NOT NULL,
    exchange TEXT NOT NULL,
    pair TEXT NOT NULL,
    side TEXT NOT NULL,           -- 'long' | 'short'
    entry_price REAL NOT NULL,
    exit_price REAL,
    quantity REAL NOT NULL,
    entry_time TEXT NOT NULL,     -- ISO 8601
    exit_time TEXT,
    pnl REAL,
    pnl_pct REAL,
    fee REAL DEFAULT 0,
    is_backtest INTEGER DEFAULT 0,
    backtest_id TEXT,
    notes TEXT,
    created_at TEXT NOT NULL
);

-- Strategies metadata
CREATE TABLE strategies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    file_path TEXT NOT NULL,
    params_json TEXT,            -- JSON blob of strategy parameters
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Backtest results
CREATE TABLE backtests (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    strategy_id TEXT NOT NULL,
    config_json TEXT NOT NULL,   -- Exchange, pair, timeframe, date range, etc.
    stats_json TEXT NOT NULL,    -- Return, Sharpe, drawdown, win rate, etc.
    equity_curve_json TEXT,      -- Array of {time, equity} points
    created_at TEXT NOT NULL
);

-- Exchange connections
CREATE TABLE exchanges (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    exchange_type TEXT NOT NULL,  -- 'cex' | 'dex'
    provider TEXT NOT NULL,       -- 'binance' | 'bybit' | 'uniswap' | etc.
    config_encrypted TEXT NOT NULL, -- Encrypted JSON with API keys / wallet info
    is_active INTEGER DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Bot state
CREATE TABLE bot_state (
    id TEXT PRIMARY KEY DEFAULT 'singleton',
    status TEXT NOT NULL DEFAULT 'stopped',  -- 'running' | 'stopped' | 'error'
    strategy_id TEXT,
    exchange_id TEXT,
    pair TEXT,
    started_at TEXT,
    config_json TEXT
);

-- Equity snapshots (for charting)
CREATE TABLE equity_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    equity REAL NOT NULL,
    source TEXT NOT NULL DEFAULT 'live'  -- 'live' | backtest_id
);
```

### File System Layout

```
~/.quantalgo/
├── quantalgo.db              # SQLite database
├── strategies/               # Python strategy files
│   ├── sma_crossover.py
│   ├── rsi_mean_revert.py
│   └── ...
├── backtests/                # Cached backtest data (large datasets)
│   └── ...
├── logs/                     # Archived bot logs
│   └── bot_2026-04-09.log
└── config.json               # App settings (non-sensitive)
```

---

## Rust Backend (Tauri Commands)

All business logic and exchange communication lives in Rust. The frontend calls these via `invoke()`.

### Strategy Commands
```rust
#[tauri::command] fn list_strategies() -> Vec<Strategy>;
#[tauri::command] fn get_strategy(id: String) -> Strategy;
#[tauri::command] fn create_strategy(name: String, description: String) -> Strategy;
#[tauri::command] fn save_strategy(id: String, code: String, params: Value) -> Strategy;
#[tauri::command] fn delete_strategy(id: String) -> bool;
#[tauri::command] fn read_strategy_file(id: String) -> String;  // Returns .py content
```

### Backtest Commands
```rust
#[tauri::command] fn run_backtest(config: BacktestConfig) -> BacktestResult;
#[tauri::command] fn list_backtests() -> Vec<BacktestSummary>;
#[tauri::command] fn get_backtest(id: String) -> BacktestResult;
#[tauri::command] fn save_backtest(result: BacktestResult, name: String) -> String;
#[tauri::command] fn delete_backtest(id: String) -> bool;
```

### Bot Commands
```rust
#[tauri::command] fn start_bot(strategy_id: String, exchange_id: String, pair: String, config: Value) -> BotStatus;
#[tauri::command] fn stop_bot() -> BotStatus;
#[tauri::command] fn get_bot_status() -> BotStatus;
#[tauri::command] fn get_bot_logs(limit: usize, offset: usize) -> Vec<LogEntry>;
```

### Exchange Commands
```rust
#[tauri::command] fn list_exchanges() -> Vec<Exchange>;
#[tauri::command] fn add_exchange(config: ExchangeConfig) -> Exchange;
#[tauri::command] fn update_exchange(id: String, config: ExchangeConfig) -> Exchange;
#[tauri::command] fn delete_exchange(id: String) -> bool;
#[tauri::command] fn test_exchange_connection(id: String) -> ConnectionResult;
#[tauri::command] fn get_balances(exchange_id: String) -> Vec<Balance>;
#[tauri::command] fn get_exchange_pairs(exchange_id: String) -> Vec<String>;
```

### Trade/Journal Commands
```rust
#[tauri::command] fn list_trades(filters: TradeFilters) -> Vec<Trade>;
#[tauri::command] fn get_trade_stats(filters: TradeFilters) -> TradeStats;
#[tauri::command] fn update_trade_notes(id: String, notes: String) -> Trade;
#[tauri::command] fn get_equity_curve(source: String, timeframe: String) -> Vec<EquityPoint>;
```

### Settings Commands
```rust
#[tauri::command] fn get_settings() -> AppSettings;
#[tauri::command] fn update_settings(settings: AppSettings) -> AppSettings;
#[tauri::command] fn detect_python() -> Option<String>;  // Auto-detect Python path
```

---

## Tauri Event System

Real-time communication from Rust to frontend via Tauri events:

| Event Name | Payload | Description |
|---|---|---|
| `bot:log` | `{ timestamp, level, message }` | Bot log line → xterm terminal |
| `bot:trade` | `{ trade: Trade }` | Trade opened or closed |
| `bot:status` | `{ status, strategy_id }` | Bot status changed |
| `bot:equity` | `{ timestamp, equity }` | Equity snapshot for live chart |
| `bot:error` | `{ message, details }` | Bot error occurred |
| `backtest:progress` | `{ pct, message }` | Backtest progress update |
| `backtest:complete` | `{ result: BacktestResult }` | Backtest finished |

Frontend listens via `listen()` from `@tauri-apps/api/event`.

---

## Pinia Stores

### `stores/bot.ts`
- State: `status`, `activeStrategyId`, `activeExchangeId`, `activePair`, `uptime`, `openPositions`, `recentLogs`
- Actions: `start()`, `stop()`, `refreshStatus()`
- Subscribes to Tauri events: `bot:status`, `bot:trade`, `bot:log`, `bot:equity`

### `stores/strategies.ts`
- State: `strategies[]`, `activeStrategyId`, `editorContent`
- Actions: `load()`, `create()`, `save()`, `delete()`, `select()`

### `stores/backtest.ts`
- State: `backtests[]`, `activeBacktestId`, `currentResult`, `isRunning`, `progress`
- Actions: `run()`, `load()`, `save()`, `delete()`, `compare(ids[])`

### `stores/journal.ts`
- State: `trades[]`, `stats`, `filters`
- Actions: `loadTrades()`, `loadStats()`, `updateFilters()`, `updateNotes()`

### `stores/exchange.ts`
- State: `exchanges[]`, `activeExchangeId`, `balances`
- Actions: `load()`, `add()`, `update()`, `delete()`, `testConnection()`, `refreshBalances()`

### `stores/app.ts`
- State: `theme`, `fontSize`, `settings`
- Actions: `applyTheme()`, `applyFontSize()`, `loadSettings()`, `saveSettings()`

---

## Component Structure

```
app/
├── app.vue                          # Root: theme init, global keybinds, NuxtPage
├── assets/
│   ├── css/main.css                 # Theme vars, base styles, scrollbars, xterm, monaco
│   └── images/
│       └── QuantAlgo.png            # Text logo for sidebar
├── components/
│   ├── Layout/
│   │   ├── Sidebar.vue              # Left nav sidebar
│   │   ├── TopStatusBar.vue         # Bot status + metrics bar
│   │   ├── RightSidebar.vue         # Context-sensitive right panel
│   │   └── ThemeToggle.vue          # Light/dark pill switch
│   ├── Strategy/
│   │   ├── StrategyEditor.vue       # Monaco editor wrapper for .py files
│   │   ├── StrategyList.vue         # Strategy cards/list
│   │   └── StrategyToolbar.vue      # Save, run backtest, deploy buttons
│   ├── Backtest/
│   │   ├── BacktestConfig.vue       # Configuration form
│   │   ├── BacktestResults.vue      # Stats grid + charts + trade table
│   │   ├── BacktestComparison.vue   # Multi-backtest overlay
│   │   └── EquityCurveChart.vue     # Lightweight Charts wrapper
│   ├── Terminal/
│   │   └── BotTerminal.vue          # xterm.js instance for bot logs
│   ├── Journal/
│   │   ├── TradeTable.vue           # Sortable/filterable trade list
│   │   ├── StatsHeader.vue          # Key metrics row
│   │   └── CalendarHeatmap.vue      # Daily PnL calendar
│   ├── Charts/
│   │   ├── EquityCurve.vue          # Main equity line chart
│   │   ├── DrawdownChart.vue        # Drawdown area chart
│   │   └── TradeMarkers.vue         # Entry/exit markers on chart
│   ├── Exchange/
│   │   ├── ExchangeList.vue         # Connected exchanges
│   │   ├── ExchangeForm.vue         # Add/edit exchange modal
│   │   └── BalanceDisplay.vue       # Account balances
│   └── UI/
│       ├── StatusDot.vue            # Animated status indicator
│       ├── MetricPill.vue           # Rounded metric chip
│       ├── SettingsModal.vue        # Settings page sections
│       └── ConfirmModal.vue         # Confirmation dialog
├── composables/
│   ├── useTheme.ts                  # Theme toggle + persistence
│   ├── useTauriEvent.ts             # Helper for Tauri event subscriptions
│   └── usePython.ts                 # Python process management helpers
├── layouts/
│   └── default.vue                  # Three-column layout shell
├── pages/
│   ├── index.vue                    # Dashboard
│   ├── strategies.vue               # Strategy editor
│   ├── backtest.vue                 # Backtester
│   ├── terminal.vue                 # Bot terminal
│   ├── journal.vue                  # Trade journal
│   ├── charts.vue                   # Equity & trade charts
│   ├── exchange.vue                 # Exchange management
│   └── settings.vue                 # App settings
├── types/
│   └── index.ts                     # Shared TypeScript interfaces
└── utils/
    └── format.ts                    # Number/date/PnL formatting helpers
```

---

## Nuxt Configuration

```ts
// nuxt.config.ts
import tailwindcss from '@tailwindcss/vite'
import pkg from './package.json'

export default defineNuxtConfig({
  ssr: false,
  devtools: { enabled: false },

  runtimeConfig: {
    public: {
      appVersion: pkg.version,
    },
  },

  srcDir: 'app',

  modules: [
    '@pinia/nuxt',
    '@vueuse/nuxt',
  ],

  css: [
    '~/assets/css/main.css',
  ],

  app: {
    head: {
      title: 'QuantAlgo',
      meta: [
        { name: 'description', content: 'Crypto Trading Bot Terminal' },
      ],
    },
  },

  vite: {
    plugins: [tailwindcss()],
    optimizeDeps: {
      include: ['monaco-editor', 'lightweight-charts'],
    },
    clearScreen: false,
    envPrefix: ['VITE_', 'TAURI_'],
  },

  devServer: {
    port: 1420,
  },

  compatibilityDate: '2025-01-01',
})
```

---

## Package Dependencies

### package.json
```json
{
  "name": "quantalgo",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "nuxt dev",
    "build": "nuxt build",
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build",
    "version:sync": "node scripts/sync-version.js"
  },
  "dependencies": {
    "@pinia/nuxt": "^0.9.0",
    "@tauri-apps/api": "^2.2.0",
    "@tauri-apps/plugin-dialog": "^2.2.0",
    "@tauri-apps/plugin-shell": "^2.2.0",
    "@vueuse/core": "^12.0.0",
    "@vueuse/nuxt": "^12.0.0",
    "@xterm/addon-fit": "^0.10.0",
    "@xterm/addon-unicode11": "^0.9.0",
    "@xterm/addon-webgl": "^0.18.0",
    "@xterm/xterm": "^5.5.0",
    "lightweight-charts": "^4.2.0",
    "monaco-editor": "^0.52.0",
    "nuxt": "^3.15.0",
    "pinia": "^2.3.0",
    "uuid": "^11.0.0",
    "vue": "^3.5.0"
  },
  "devDependencies": {
    "@tailwindcss/vite": "^4.0.0",
    "@tauri-apps/cli": "^2.2.0",
    "@types/uuid": "^10.0.0",
    "tailwindcss": "^4.0.0",
    "typescript": "^5.7.0",
    "vue-tsc": "^2.0.0"
  }
}
```

### Cargo.toml (src-tauri/)
```toml
[package]
name = "quantalgo"
version = "0.1.0"
edition = "2021"

[lib]
name = "quantalgo_lib"
crate-type = ["lib", "cdylib", "staticlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
portable-pty = "0.8"
rusqlite = { version = "0.31", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
uuid = { version = "1", features = ["v4"] }
dirs = "5"
aes-gcm = "0.10"                # Encrypt API keys at rest
ring = "0.17"                    # Key derivation for encryption
hmac = "0.12"                    # Exchange API request signing
sha2 = "0.10"                    # Exchange API request signing
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
```

---

## Python Strategy SDK

A minimal Python package (`quantalgo/`) bundled with the app or installed via pip, providing the base class and utilities strategies import.

### Core API

```python
# quantalgo/strategy.py
class Strategy:
    params: dict                   # Overridable parameters
    def on_candle(self, candle): ...
    def on_tick(self, tick): ...
    def on_trade(self, trade): ...
    def on_start(self): ...
    def on_stop(self): ...

    # Order methods (available to strategies)
    def buy(self, pair, quantity, price=None): ...      # Market or limit
    def sell(self, pair, quantity, price=None): ...
    def close(self, position_id=None): ...              # Close position
    def cancel(self, order_id): ...

    # Data access
    def get_position(self, pair=None): ...
    def get_balance(self, asset=None): ...
    def get_candles(self, pair, timeframe, limit): ...

    # Indicators (convenience wrappers)
    def sma(self, series, period): ...
    def ema(self, series, period): ...
    def rsi(self, series, period): ...
    def macd(self, series, fast, slow, signal): ...
    def bollinger(self, series, period, std): ...
    def atr(self, candles, period): ...
```

### Communication Protocol (Rust ↔ Python)

JSON-RPC over stdin/stdout of the child process:

```json
// Rust → Python: feed candle
{"method": "on_candle", "params": {"open": 100, "high": 105, "low": 99, "close": 103, "volume": 1234, "time": "2026-04-09T12:00:00Z"}}

// Python → Rust: place order
{"method": "buy", "params": {"pair": "BTC/USDT", "quantity": 0.01, "price": null}}

// Python → Rust: log message
{"method": "log", "params": {"level": "info", "message": "SMA crossover detected"}}

// Rust → Python: order filled
{"method": "on_trade", "params": {"id": "...", "pair": "BTC/USDT", "side": "long", "price": 64250.0, "quantity": 0.01}}
```

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+S` | Save current strategy |
| `Ctrl+B` | Toggle left sidebar |
| `Ctrl+Shift+B` | Toggle right sidebar |
| `Ctrl+Enter` | Start/stop bot (from any page) |
| `Ctrl+Shift+Enter` | Run backtest on current strategy |
| `Ctrl+Tab` | Cycle between open strategy tabs |
| `Ctrl+J` | Toggle terminal panel |

---

## Implementation Order

1. **Project scaffold:** `npm create tauri-app`, Nuxt config, Tailwind, CSS tokens, folder structure
2. **Layout shell:** `default.vue` (3-column), Sidebar, TopStatusBar, RightSidebar, ThemeToggle
3. **Settings page + app store:** Theme persistence, settings CRUD, Rust settings commands
4. **SQLite setup:** Rust database initialization, migrations, basic CRUD commands
5. **Strategy editor:** Monaco integration, strategy CRUD (Rust + Pinia), file read/write
6. **Python runtime:** Rust child process spawning, JSON-RPC protocol, strategy loading/execution
7. **Terminal page:** xterm.js integration, Tauri event streaming, log rendering
8. **Backtest engine:** Python backtest runner, progress events, result storage, equity chart
9. **Journal page:** Trade table, stats calculations, calendar heatmap
10. **Charts page:** Lightweight Charts integration, equity curve, trade markers, drawdown
11. **Exchange integration:** Connection management, encrypted key storage, balance fetching
12. **Live trading:** Bot start/stop, order routing through exchange APIs, position tracking
13. **Dashboard:** Aggregated overview cards, quick actions
14. **Polish:** Notifications, error handling, keyboard shortcuts, responsive panels
