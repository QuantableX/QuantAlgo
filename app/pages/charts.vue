<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { BotEquityEvent, BotTradeEvent, EquityPoint, Trade } from '~/types'

// State
const route = useRoute()
const router = useRouter()
const timeframes = ['1h', '4h', '1d', '1w', '1M', 'All'] as const
type ChartTimeframe = typeof timeframes[number]

function routeTimeframe(): ChartTimeframe {
  const tf = route.query.tf
  return typeof tf === 'string' && timeframes.includes(tf as ChartTimeframe)
    ? tf as ChartTimeframe
    : '1d'
}

const selectedTimeframe = ref<ChartTimeframe>(routeTimeframe())
const equityData = ref<EquityPoint[]>([])
const trades = ref<Trade[]>([])
const isLoading = ref(false)
const error = ref<string | null>(null)
let unlistenEquity: UnlistenFn | null = null
let unlistenTrade: UnlistenFn | null = null

// Compute drawdown data from equity curve
const drawdownData = computed<EquityPoint[]>(() => {
  if (!equityData.value.length) return []

  const firstPoint = equityData.value[0]
  if (!firstPoint) return []

  let peak = firstPoint.equity
  return equityData.value.map((point) => {
    if (point.equity > peak) peak = point.equity
    const drawdown = peak > 0 ? ((point.equity - peak) / peak) * 100 : 0
    return {
      time: point.time,
      equity: drawdown,
    }
  })
})

async function fetchEquityCurve() {
  isLoading.value = true
  error.value = null
  try {
    equityData.value = await invoke<EquityPoint[]>('get_equity_curve', {
      source: 'paper',
      timeframe: selectedTimeframe.value,
    })
  } catch (err) {
    console.error('[charts] Failed to load equity curve:', err)
    error.value = String(err)
    equityData.value = []
  } finally {
    isLoading.value = false
  }
}

async function fetchTrades() {
  try {
    trades.value = await invoke<Trade[]>('list_trades', {
      filters: { is_backtest: false, limit: 100 },
    })
  } catch (err) {
    console.error('[charts] Failed to load trades:', err)
  }
}

function selectTimeframe(tf: string) {
  selectedTimeframe.value = tf as ChartTimeframe
  router.replace({
    path: route.path,
    query: {
      ...route.query,
      tf,
    },
  })
}

watch(selectedTimeframe, () => {
  fetchEquityCurve()
})

watch(
  () => route.query.tf,
  () => {
    const next = routeTimeframe()
    if (next !== selectedTimeframe.value) {
      selectedTimeframe.value = next
    }
  },
)

onMounted(async () => {
  await Promise.all([fetchEquityCurve(), fetchTrades()])
  unlistenEquity = await listen<BotEquityEvent>('bot:equity', (event) => {
    equityData.value = [
      ...equityData.value,
      {
        time: event.payload.timestamp,
        equity: event.payload.equity,
      },
    ].slice(-1000)
  })
  unlistenTrade = await listen<BotTradeEvent>('bot:trade', async () => {
    await fetchTrades()
  })
})

onUnmounted(() => {
  unlistenEquity?.()
  unlistenTrade?.()
})
</script>

<template>
  <div class="charts">
    <!-- Timeframe Selector -->
    <div class="charts__toolbar">
      <h2 class="charts__title">Equity & Performance</h2>
      <div class="timeframe-bar">
        <button
          v-for="tf in timeframes"
          :key="tf"
          class="btn btn-sm"
          :class="{ 'btn-primary': selectedTimeframe === tf }"
          @click="selectTimeframe(tf)"
        >
          {{ tf }}
        </button>
      </div>
    </div>

    <!-- Loading State -->
    <div v-if="isLoading && !equityData.length" class="charts__loading">
      <p class="text-muted">Loading chart data...</p>
    </div>

    <!-- Error State -->
    <div v-else-if="error && !equityData.length" class="charts__error card">
      <p class="text-error">Failed to load chart data</p>
      <p class="text-muted">{{ error }}</p>
      <button class="btn btn-sm" @click="fetchEquityCurve">Retry</button>
    </div>

    <!-- Empty State -->
    <div v-else-if="!equityData.length && !isLoading" class="charts__empty">
      <p class="empty-state text-muted">
        No equity data available. Start trading to see your performance charts.
      </p>
    </div>

    <!-- Charts -->
    <template v-else>
      <!-- Main Equity Curve -->
      <div class="charts__main card">
        <div class="chart-header">
          <h3 class="chart-header__title">Equity Curve</h3>
          <span v-if="isLoading" class="text-muted chart-header__status">Updating...</span>
        </div>
        <div class="chart-container">
          <EquityCurve :data="equityData" />
        </div>
      </div>

      <!-- Drawdown Chart -->
      <div class="charts__secondary card">
        <div class="chart-header">
          <h3 class="chart-header__title">Drawdown</h3>
        </div>
        <div class="chart-container chart-container--small">
          <DrawdownChart :data="drawdownData" />
        </div>
      </div>

      <!-- Trade Markers Info -->
      <div v-if="trades.length" class="charts__trades card">
        <div class="chart-header">
          <h3 class="chart-header__title">
            Recent Trades
            <span class="pill">{{ trades.length }}</span>
          </h3>
        </div>
        <div class="trades-summary">
          <div class="trades-summary__grid">
            <div
              v-for="trade in trades.slice(0, 10)"
              :key="trade.id"
              class="trade-marker-item"
            >
              <span
                class="trade-marker-item__side"
                :class="trade.side === 'long' ? 'text-accent' : 'text-error'"
              >
                {{ trade.side === 'long' ? 'L' : 'S' }}
              </span>
              <span class="trade-marker-item__pair">{{ trade.pair }}</span>
              <span
                class="trade-marker-item__pnl mono"
                :class="(trade.pnl ?? 0) >= 0 ? 'text-success' : 'text-error'"
              >
                {{ trade.pnl != null ? (trade.pnl >= 0 ? '+' : '') + trade.pnl.toFixed(2) : '--' }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.charts {
  height: 100%;
  overflow-y: auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.charts__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
}

.charts__title {
  font-size: 16px;
  font-weight: 600;
  color: var(--qa-text);
}

.timeframe-bar {
  display: flex;
  gap: 4px;
}

.charts__loading,
.charts__empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 300px;
  font-size: 14px;
}

.charts__error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 40px;
  text-align: center;
}

.empty-state {
  font-size: 13px;
}

/* Chart Cards */
.charts__main {
  flex: 1;
  min-height: 300px;
  display: flex;
  flex-direction: column;
}

.charts__secondary {
  flex-shrink: 0;
}

.charts__trades {
  flex-shrink: 0;
}

.chart-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.chart-header__title {
  font-size: 13px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--qa-text-secondary);
  display: flex;
  align-items: center;
  gap: 8px;
}

.chart-header__status {
  font-size: 12px;
}

.chart-container {
  flex: 1;
  min-height: 250px;
  position: relative;
}

.chart-container--small {
  min-height: 150px;
  height: 150px;
}

/* Trade Markers */
.trades-summary__grid {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.trade-marker-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 0;
  border-bottom: 1px solid var(--qa-border-subtle);
  font-size: 13px;
}

.trade-marker-item:last-child {
  border-bottom: none;
}

.trade-marker-item__side {
  width: 20px;
  font-weight: 700;
  font-size: 12px;
  text-align: center;
}

.trade-marker-item__pair {
  flex: 1;
  color: var(--qa-text);
}

.trade-marker-item__pnl {
  font-weight: 600;
  font-size: 13px;
}
</style>
