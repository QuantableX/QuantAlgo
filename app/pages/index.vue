<script setup lang="ts">
import { useBotStore } from '~/stores/bot'
import { useStrategiesStore } from '~/stores/strategies'
import { useExchangeStore } from '~/stores/exchange'
import { formatCurrency, formatPnl, formatPrice, formatTime, formatDuration } from '~/utils/format'
import type { LogEntry } from '~/types'

const router = useRouter()
const botStore = useBotStore()
const strategiesStore = useStrategiesStore()
const exchangeStore = useExchangeStore()

// Computed: active strategy name
const activeStrategyName = computed(() => {
  if (!botStore.activeStrategyId) return 'None'
  const s = strategiesStore.strategies.find((s) => s.id === botStore.activeStrategyId)
  return s?.name ?? 'Unknown'
})

// Computed: uptime formatted
const uptimeFormatted = computed(() => {
  if (!botStore.startedAt || botStore.status !== 'running') return '--'
  const seconds = Math.floor((Date.now() - new Date(botStore.startedAt).getTime()) / 1000)
  return formatDuration(seconds)
})

// Keep uptime ticking
const now = ref(Date.now())
let uptimeInterval: ReturnType<typeof setInterval> | null = null

// Status dot class
const statusDotClass = computed(() => {
  switch (botStore.status) {
    case 'running': return 'status-dot--running'
    case 'error': return 'status-dot--error'
    default: return 'status-dot--stopped'
  }
})

const statusLabel = computed(() => {
  switch (botStore.status) {
    case 'running': return `Running (${botStore.modeLabel})`
    case 'error': return 'Error'
    default: return 'Stopped'
  }
})

// PnL rows
const pnlRows = computed(() => [
  { label: 'Today', value: botStore.pnl.today },
  { label: '7 Day', value: botStore.pnl.week },
  { label: '30 Day', value: botStore.pnl.month },
  { label: 'All Time', value: botStore.pnl.all },
])

// Recent logs (last 10)
const recentLogs = computed(() => {
  return botStore.recentLogs.slice(-10).reverse()
})

function logLevelClass(level: LogEntry['level']): string {
  switch (level) {
    case 'trade': return 'text-accent'
    case 'warn': return 'text-warning'
    case 'error': return 'text-error'
    default: return ''
  }
}

// Current mark price for an open position
function currentPrice(pos: { pair: string; entry_price: number; exit_price: number | null }): number {
  // For open positions, use the latest mark price from the bot store if available.
  if (pos.exit_price !== null) return pos.exit_price
  if (botStore.lastPrice > 0 && botStore.activePair === pos.pair) return botStore.lastPrice
  return pos.entry_price
}

// Position PnL (unrealized)
function unrealizedPnl(entry: number, current: number, qty: number, side: 'long' | 'short'): number {
  if (side === 'long') return (current - entry) * qty
  return (entry - current) * qty
}

function positionAmount(entry: number, qty: number): number {
  return entry * qty
}

// Deploy modal (for starting from dashboard)
const showDeployModal = ref(false)
const isSavingStrategy = ref(false)
const saveBeforeDeployError = ref<string | null>(null)

// Actions
async function handleStartStop() {
  if (botStore.isRunning) {
    await botStore.stop()
  } else if (strategiesStore.activeStrategyId) {
    saveBeforeDeployError.value = null
    showDeployModal.value = true
  } else {
    router.push('/strategies')
  }
}

function handleDeployStarted() {
  showDeployModal.value = false
}

async function handleSaveFirst() {
  if (!strategiesStore.activeStrategyId) return
  isSavingStrategy.value = true
  saveBeforeDeployError.value = null
  try {
    await strategiesStore.save(
      strategiesStore.activeStrategyId,
      strategiesStore.editorContent,
      strategiesStore.paramsContent,
    )
  } catch (err) {
    saveBeforeDeployError.value = String(err)
    console.error('Failed to save strategy before deploy:', err)
  } finally {
    isSavingStrategy.value = false
  }
}

function navigateTo(path: string) {
  router.push(path)
}

onMounted(() => {
  // Stores are bootstrapped in default.vue layout — no need to re-init here
  uptimeInterval = setInterval(() => {
    now.value = Date.now()
  }, 1000)
})

onUnmounted(() => {
  if (uptimeInterval) clearInterval(uptimeInterval)
})
</script>

<template>
  <div class="dashboard">
    <div class="dashboard__grid">
      <!-- Card 1: Bot Status -->
      <div class="card">
        <h3 class="card__title">Bot Status</h3>
        <div class="status-block">
          <div class="status-indicator">
            <span class="status-dot" :class="statusDotClass" />
            <span class="status-label">{{ statusLabel }}</span>
          </div>
          <div class="status-details">
            <div class="status-row">
              <span class="status-row__label">Strategy</span>
              <span class="status-row__value">{{ activeStrategyName }}</span>
            </div>
            <div class="status-row">
              <span class="status-row__label">Pair</span>
              <span class="status-row__value">{{ botStore.activePair ?? '--' }}</span>
            </div>
            <div class="status-row">
              <span class="status-row__label">Mode</span>
              <span
                class="status-row__value"
                :class="botStore.isPaper ? '' : 'text-warning'"
              >
                {{ botStore.modeLabel }}
              </span>
            </div>
            <div class="status-row">
              <span class="status-row__label">Uptime</span>
              <!-- Force reactivity via now ref -->
              <span class="status-row__value">{{ now && uptimeFormatted }}</span>
            </div>
          </div>
          <button
            class="btn"
            :class="botStore.isRunning ? 'btn-danger' : 'btn-success'"
            @click="handleStartStop"
          >
            {{ botStore.isRunning ? 'Stop Bot' : 'Start Bot' }}
          </button>
        </div>
      </div>

      <!-- Card 2: PnL Summary -->
      <div class="card">
        <h3 class="card__title">Profit &amp; Loss</h3>
        <div class="pnl-list">
          <div v-for="row in pnlRows" :key="row.label" class="pnl-row">
            <span class="pnl-row__label">{{ row.label }}</span>
            <span class="pnl-row__value" :class="formatPnl(row.value).class">
              {{ formatPnl(row.value).text }}
            </span>
          </div>
        </div>
      </div>

      <!-- Card 3: Open Positions -->
      <div class="card">
        <h3 class="card__title">
          Open Positions
          <span v-if="botStore.openPositions.length" class="pill">
            {{ botStore.openPositions.length }}
          </span>
        </h3>
        <div v-if="botStore.openPositions.length" class="positions-table-wrap">
          <table class="table">
            <thead>
              <tr>
                <th>Pair</th>
                <th>Side</th>
                <th>Amount</th>
                <th>Entry</th>
                <th>Current</th>
                <th>Unrealized PnL</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="pos in botStore.openPositions" :key="pos.id">
                <td>{{ pos.pair }}</td>
                <td :class="pos.side === 'long' ? 'text-accent' : 'text-error'">
                  {{ pos.side.toUpperCase() }}
                </td>
                <td>{{ formatCurrency(positionAmount(pos.entry_price, pos.quantity)) }}</td>
                <td>{{ formatPrice(pos.entry_price) }}</td>
                <td>{{ formatPrice(currentPrice(pos)) }}</td>
                <td
                  :class="unrealizedPnl(pos.entry_price, currentPrice(pos), pos.quantity, pos.side) >= 0
                    ? 'text-success'
                    : 'text-error'"
                >
                  {{ formatPnl(unrealizedPnl(pos.entry_price, currentPrice(pos), pos.quantity, pos.side)).text }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-else class="empty-state text-muted">No open positions</p>
      </div>

      <!-- Card 4: Recent Activity -->
      <div class="card">
        <h3 class="card__title">Recent Activity</h3>
        <div v-if="recentLogs.length" class="activity-list">
          <div v-for="(log, idx) in recentLogs" :key="idx" class="activity-entry">
            <span class="activity-entry__time text-muted">{{ formatTime(log.timestamp) }}</span>
            <span class="activity-entry__msg" :class="logLevelClass(log.level)">
              {{ log.message }}
            </span>
          </div>
        </div>
        <p v-else class="empty-state text-muted">No recent activity</p>
      </div>
    </div>

    <!-- Deploy Modal -->
    <DeployModal
      :visible="showDeployModal"
      :strategy-id="strategiesStore.activeStrategyId"
      :is-dirty="strategiesStore.isDirty || isSavingStrategy"
      :save-error="saveBeforeDeployError"
      @close="showDeployModal = false"
      @save-first="handleSaveFirst"
      @started="handleDeployStarted"
    />

    <!-- Quick Actions -->
    <div class="quick-actions">
      <button class="btn btn-success" @click="handleStartStop">
        {{ botStore.isRunning ? 'Stop Bot' : 'Start Bot' }}
      </button>
      <button class="btn" @click="navigateTo('/backtest')">Open Backtest</button>
      <button class="btn" @click="navigateTo('/strategies')">New Strategy</button>
    </div>
  </div>
</template>

<style scoped>
.dashboard {
  height: 100%;
  overflow-y: auto;
  padding: 20px;
}

.dashboard__grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}

@media (max-width: 900px) {
  .dashboard__grid {
    grid-template-columns: 1fr;
  }
}

.card__title {
  font-size: 13px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--qa-text-secondary);
  margin-bottom: 16px;
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Bot Status */
.status-block {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.status-indicator {
  display: flex;
  align-items: center;
  gap: 10px;
}

.status-dot {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-dot--running {
  background: var(--qa-accent);
  animation: pulse-dot 2s ease-in-out infinite;
}

.status-dot--stopped {
  background: var(--qa-text-muted);
}

.status-dot--error {
  background: var(--qa-error);
  animation: pulse-dot 2s ease-in-out infinite;
}

.status-label {
  font-size: 18px;
  font-weight: 600;
  color: var(--qa-text);
}

.status-details {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.status-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.status-row__label {
  font-size: 13px;
  color: var(--qa-text-muted);
}

.status-row__value {
  font-size: 13px;
  color: var(--qa-text);
  font-weight: 500;
}

/* PnL */
.pnl-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.pnl-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
  border-bottom: 1px solid var(--qa-border-subtle);
}

.pnl-row:last-child {
  border-bottom: none;
}

.pnl-row__label {
  font-size: 13px;
  color: var(--qa-text-secondary);
}

.pnl-row__value {
  font-size: 14px;
  font-weight: 600;
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', 'Fira Code', monospace;
}

/* Positions */
.positions-table-wrap {
  overflow-x: auto;
}

/* Activity */
.activity-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 300px;
  overflow-y: auto;
}

.activity-entry {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 6px 0;
  border-bottom: 1px solid var(--qa-border-subtle);
}

.activity-entry:last-child {
  border-bottom: none;
}

.activity-entry__time {
  font-size: 11px;
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', 'Fira Code', monospace;
  flex-shrink: 0;
}

.activity-entry__msg {
  font-size: 13px;
}

/* Empty state */
.empty-state {
  font-size: 13px;
  padding: 20px 0;
  text-align: center;
}

/* Quick Actions */
.quick-actions {
  display: flex;
  gap: 12px;
  margin-top: 20px;
  padding-top: 20px;
  border-top: 1px solid var(--qa-border-subtle);
}
</style>
