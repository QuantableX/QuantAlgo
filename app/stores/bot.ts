import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ref, computed } from 'vue'
import type {
  BotStatusType,
  BotStatus,
  LogEntry,
  Trade,
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
  const openPositions = ref<Trade[]>([])
  const recentLogs = ref<LogEntry[]>([])
  const equity = ref<number>(0)
  const pnl = ref<PnlSummary>({
    today: 0,
    week: 0,
    month: 0,
    all: 0,
  })

  const isInitialized = ref(false)
  const unlistenHandlers = ref<UnlistenFn[]>([])

  // ── Getters ──

  const isRunning = computed(() => status.value === 'running')

  const uptime = computed(() => {
    if (!startedAt.value || status.value !== 'running') return 0
    return Math.floor((Date.now() - new Date(startedAt.value).getTime()) / 1000)
  })

  // ── Actions ──

  async function start(
    strategyId: string,
    exchangeId: string,
    pair: string,
    config?: Record<string, unknown>,
  ) {
    try {
      await invoke('start_bot', {
        strategyId,
        exchangeId,
        pair,
        config: config ? JSON.stringify(config) : null,
      })
      status.value = 'running'
      activeStrategyId.value = strategyId
      activeExchangeId.value = exchangeId
      activePair.value = pair
      startedAt.value = new Date().toISOString()
    } catch (err) {
      console.error('[bot store] Failed to start bot:', err)
      throw err
    }
  }

  async function stop() {
    try {
      await invoke('stop_bot')
      status.value = 'stopped'
      startedAt.value = null
    } catch (err) {
      console.error('[bot store] Failed to stop bot:', err)
      throw err
    }
  }

  async function refreshStatus() {
    try {
      const result = await invoke<BotStatus>('get_bot_status')
      status.value = result.status
      activeStrategyId.value = result.strategy_id
      activeExchangeId.value = result.exchange_id
      activePair.value = result.pair
      startedAt.value = result.started_at
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

      const unlistenStatus = await listen<BotStatusEvent>('bot:status', (event) => {
        status.value = event.payload.status
        activeStrategyId.value = event.payload.strategy_id
        if (event.payload.status === 'stopped') {
          startedAt.value = null
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
        // Add to open positions if no exit, remove if closed
        if (trade.exit_price === null) {
          openPositions.value.push(trade)
        } else {
          openPositions.value = openPositions.value.filter((t) => t.id !== trade.id)
          // Update PnL if trade has realized pnl
          if (trade.pnl !== null) {
            pnl.value.today += trade.pnl
            pnl.value.all += trade.pnl
          }
        }
      })

      const unlistenEquity = await listen<BotEquityEvent>('bot:equity', (event) => {
        equity.value = event.payload.equity
      })

      const unlistenError = await listen<BotErrorEvent>('bot:error', (event) => {
        addLog({
          timestamp: new Date().toISOString(),
          level: 'error',
          message: event.payload.message,
        })
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
    openPositions,
    recentLogs,
    equity,
    pnl,
    // Getters
    isRunning,
    uptime,
    // Actions
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
