import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ref, computed } from 'vue'
import type {
  BotStatusType,
  BotStatus,
  LogEntry,
  Trade,
  TradingMode,
  PreflightResult,
  BotLogEvent,
  BotTradeEvent,
  BotStatusEvent,
  BotEquityEvent,
  BotErrorEvent,
} from '~/types'

const MAX_LOG_ENTRIES = 500

interface PnlSummary {
  today: number
  week: number
  month: number
  all: number
}

export const useBotStore = defineStore('bot', () => {
  // ── State ──

  const status = ref<BotStatusType>('stopped')
  const activeStrategyId = ref<string | null>(null)
  const activeExchangeId = ref<string | null>(null)
  const activePair = ref<string | null>(null)
  const startedAt = ref<string | null>(null)
  const tradingMode = ref<TradingMode>('paper')
  const openPositions = ref<Trade[]>([])
  const recentLogs = ref<LogEntry[]>([])
  const equity = ref<number>(0)
  const lastPrice = ref<number>(0)
  const paperBalance = ref<number>(0)
  const pnl = ref<PnlSummary>({
    today: 0,
    week: 0,
    month: 0,
    all: 0,
  })
  const lastError = ref<string | null>(null)

  const isInitialized = ref(false)
  const unlistenHandlers = ref<UnlistenFn[]>([])

  // ── Getters ──

  const isRunning = computed(() => status.value === 'running')
  const isPaper = computed(() => tradingMode.value === 'paper')
  const modeLabel = computed(() => tradingMode.value === 'paper' ? 'Paper' : 'Live')

  const uptime = computed(() => {
    if (!startedAt.value || status.value !== 'running') return 0
    return Math.floor((Date.now() - new Date(startedAt.value).getTime()) / 1000)
  })

  function applyStatus(result: BotStatus) {
    status.value = result.status
    activeStrategyId.value = result.strategy_id
    activeExchangeId.value = result.exchange_id
    activePair.value = result.pair
    startedAt.value = result.started_at
    tradingMode.value = result.trading_mode ?? 'paper'
    if (result.status !== 'error') {
      lastError.value = null
    }
  }

  function upsertOpenPosition(trade: Trade) {
    const existingIdx = openPositions.value.findIndex((position) => position.id === trade.id)

    if (trade.exit_price === null) {
      if (existingIdx === -1) {
        openPositions.value.push(trade)
      } else {
        openPositions.value.splice(existingIdx, 1, trade)
      }
      return
    }

    if (existingIdx !== -1) {
      openPositions.value.splice(existingIdx, 1)
    }
  }

  // ── Actions ──

  async function runPreflight(
    strategyId: string,
    exchangeId: string,
    pair: string,
    mode: TradingMode,
    config?: Record<string, unknown>,
  ): Promise<PreflightResult> {
    try {
      return await invoke<PreflightResult>('validate_bot_deploy', {
        strategyId,
        exchangeId,
        pair,
        tradingMode: mode,
        config: config ?? null,
      })
    } catch (err) {
      console.error('[bot store] Preflight failed:', err)
      throw err
    }
  }

  async function start(
    strategyId: string,
    exchangeId: string,
    pair: string,
    config?: Record<string, unknown>,
    mode: TradingMode = 'paper',
  ) {
    try {
      const result = await invoke<BotStatus>('start_bot', {
        strategyId,
        exchangeId,
        pair,
        config: config ?? null,
        tradingMode: mode,
      })
      applyStatus(result)
    } catch (err) {
      lastError.value = String(err)
      console.error('[bot store] Failed to start bot:', err)
      throw err
    }
  }

  async function stop() {
    try {
      const result = await invoke<BotStatus>('stop_bot')
      applyStatus(result)
      openPositions.value = []
    } catch (err) {
      console.error('[bot store] Failed to stop bot:', err)
      throw err
    }
  }

  async function refreshStatus() {
    try {
      const result = await invoke<BotStatus>('get_bot_status')
      applyStatus(result)
    } catch (err) {
      console.error('[bot store] Failed to refresh status:', err)
    }
  }

  async function loadLogs(limit: number = 100, offset: number = 0) {
    try {
      const logs = await invoke<LogEntry[]>('get_bot_logs', { limit, offset })
      recentLogs.value = logs
    } catch (err) {
      console.error('[bot store] Failed to load logs:', err)
    }
  }

  function addLog(entry: LogEntry) {
    recentLogs.value.push(entry)
    if (recentLogs.value.length > MAX_LOG_ENTRIES) {
      recentLogs.value = recentLogs.value.slice(-MAX_LOG_ENTRIES)
    }
  }

  function clearLogs() {
    recentLogs.value = []
  }

  // ── Event subscriptions ──

  async function init() {
    if (isInitialized.value) return
    isInitialized.value = true

    try {
      // Fetch current status on init
      await refreshStatus()
      await loadLogs(MAX_LOG_ENTRIES)

      const unlistenStatus = await listen<BotStatusEvent>('bot:status', (event) => {
        status.value = event.payload.status
        activeStrategyId.value = event.payload.strategy_id
        activeExchangeId.value = event.payload.exchange_id ?? null
        activePair.value = event.payload.pair ?? null
        startedAt.value = event.payload.started_at ?? null
        tradingMode.value = event.payload.trading_mode ?? 'paper'
        if (event.payload.status === 'stopped') {
          openPositions.value = []
          lastError.value = null
        }
      })

      const unlistenLog = await listen<BotLogEvent>('bot:log', (event) => {
        const { timestamp, level, message } = event.payload
        addLog({
          timestamp,
          level: level as LogEntry['level'],
          message,
        })
      })

      const unlistenTrade = await listen<BotTradeEvent>('bot:trade', (event) => {
        const trade = event.payload.trade
        upsertOpenPosition(trade)

        if (trade.exit_price !== null && trade.pnl !== null) {
          pnl.value.today += trade.pnl
          pnl.value.all += trade.pnl
        }
      })

      const unlistenEquity = await listen<BotEquityEvent>('bot:equity', (event) => {
        equity.value = event.payload.equity
        if (event.payload.last_price !== undefined) {
          lastPrice.value = event.payload.last_price
        }
        if (event.payload.balance !== undefined) {
          paperBalance.value = event.payload.balance
        }
      })

      const unlistenError = await listen<BotErrorEvent>('bot:error', (event) => {
        status.value = 'error'
        lastError.value = event.payload.message
        if (event.payload.details) {
          console.error('[bot store] Bot error details:', event.payload.details)
        }
      })

      unlistenHandlers.value = [
        unlistenStatus,
        unlistenLog,
        unlistenTrade,
        unlistenEquity,
        unlistenError,
      ]
    } catch (err) {
      console.error('[bot store] Failed to initialize event listeners:', err)
    }
  }

  function dispose() {
    for (const unlisten of unlistenHandlers.value) {
      unlisten()
    }
    unlistenHandlers.value = []
    isInitialized.value = false
  }

  return {
    // State
    status,
    activeStrategyId,
    activeExchangeId,
    activePair,
    startedAt,
    tradingMode,
    openPositions,
    recentLogs,
    equity,
    lastPrice,
    paperBalance,
    pnl,
    lastError,
    // Getters
    isRunning,
    isPaper,
    modeLabel,
    uptime,
    // Actions
    runPreflight,
    start,
    stop,
    refreshStatus,
    loadLogs,
    addLog,
    clearLogs,
    init,
    dispose,
  }
})
