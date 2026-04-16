// ── Strategy ──

export interface Strategy {
  id: string
  name: string
  description: string
  file_path: string
  params_json: string | null
  created_at: string
  updated_at: string
}

// ── Trade ──

export type TradeSide = 'long' | 'short'

export interface Trade {
  id: string
  strategy_id: string
  exchange: string
  pair: string
  side: TradeSide
  entry_price: number
  exit_price: number | null
  quantity: number
  entry_time: string
  exit_time: string | null
  pnl: number | null
  pnl_pct: number | null
  fee: number
  is_backtest: boolean
  backtest_id: string | null
  notes: string | null
  created_at: string
}

export interface TradeFilters {
  strategy_id?: string
  exchange?: string
  pair?: string
  side?: TradeSide
  from_date?: string
  to_date?: string
  min_pnl?: number
  is_backtest?: boolean
  backtest_id?: string
  limit?: number
  offset?: number
}

export interface TradeStats {
  total_trades: number
  win_rate: number
  avg_win: number
  avg_loss: number
  profit_factor: number
  expectancy: number
  best_trade: number
  worst_trade: number
  total_pnl: number
  total_pnl_pct: number
  avg_duration_secs: number
}

// ── Backtest ──

export interface BacktestConfig {
  strategy_id: string
  exchange: string
  pair: string
  timeframe: string
  start_date: string
  end_date: string
  initial_capital: number
  commission: number
  strategy_params?: Record<string, unknown>
}

export interface BacktestStats {
  total_return: number
  total_return_pct: number
  sharpe_ratio: number
  max_drawdown: number
  max_drawdown_pct: number
  win_rate: number
  profit_factor: number
  total_trades: number
  avg_trade_duration_secs: number
}

export interface EquityPoint {
  time: string
  equity: number
}

export interface BacktestResult {
  id: string
  name: string
  strategy_id: string
  config: BacktestConfig
  stats: BacktestStats
  equity_curve: EquityPoint[]
  trades: Trade[]
  created_at: string
}

export interface BacktestSummary {
  id: string
  name: string
  strategy_id: string
  config_json: string
  stats_json: string
  created_at: string
}

// ── Exchange ──

export type ExchangeType = 'cex' | 'dex'

export type ExchangeProvider =
  | 'binance' | 'bybit' | 'okx' | 'coinbase' | 'kraken'
  | 'uniswap' | 'jupiter' | 'hyperliquid'

export interface Exchange {
  id: string
  name: string
  exchange_type: ExchangeType
  provider: ExchangeProvider
  is_active: boolean
  created_at: string
  updated_at: string
}

export interface ExchangeConfig {
  name: string
  exchange_type: ExchangeType
  provider: ExchangeProvider
  api_key?: string
  api_secret?: string
  passphrase?: string
  wallet_address?: string
  private_key?: string
  rpc_endpoint?: string
}

export interface ConnectionResult {
  success: boolean
  message: string
  latency_ms?: number
}

export interface Balance {
  asset: string
  total: number
  available: number
  in_positions: number
}

// ── Bot ──

export type BotStatusType = 'running' | 'stopped' | 'error'
export type TradingMode = 'paper' | 'live'

export interface BotStatus {
  status: BotStatusType
  strategy_id: string | null
  exchange_id: string | null
  pair: string | null
  started_at: string | null
  config_json: string | null
  trading_mode: TradingMode
}

export interface LogEntry {
  timestamp: string
  level: 'info' | 'trade' | 'warn' | 'error'
  message: string
}

// ── Deploy / Preflight ──

export interface DeployConfig {
  strategy_id: string
  exchange_id: string
  pair: string
  trading_mode: TradingMode
  timeframe: string
  initial_balance: number
  risk_per_trade: number
  max_positions: number
  slippage: number
  fee: number
}

export type PreflightSeverity = 'ok' | 'warn' | 'error'

export interface PreflightCheck {
  id: string
  label: string
  status: PreflightSeverity
  message: string
}

export interface PreflightResult {
  checks: PreflightCheck[]
  can_start: boolean
}

// ── Settings ──

export interface AppSettings {
  theme: 'dark' | 'light'
  font_size: number
  default_pair: string
  default_timeframe: string
  python_path: string
  strategy_dir: string
  backtest_dir: string
  risk_per_trade: number
  max_concurrent_positions: number
  slippage_tolerance: number
  paper_fee_pct: number
  notify_on_trade: boolean
  notify_on_error: boolean
  notify_on_daily_summary: boolean
}

// ── Events ──

export interface BotLogEvent {
  timestamp: string
  level: string
  message: string
}

export interface BotTradeEvent {
  trade: Trade
}

export interface BotStatusEvent {
  status: BotStatusType
  strategy_id: string | null
  exchange_id: string | null
  pair: string | null
  started_at: string | null
  trading_mode: TradingMode
}

export interface BotEquityEvent {
  timestamp: string
  equity: number
  last_price?: number
  pair?: string
  balance?: number
  open_position_count?: number
}

export interface BotErrorEvent {
  message: string
  details: string | null
}

export interface BacktestProgressEvent {
  pct: number
  message: string
}

export interface BacktestCompleteEvent {
  result: BacktestResult
}
