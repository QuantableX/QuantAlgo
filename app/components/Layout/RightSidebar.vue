<script setup lang="ts">
import { useBotStore } from '~/stores/bot'
import { useStrategiesStore } from '~/stores/strategies'

const route = useRoute()
const bot = useBotStore()
const strategies = useStrategiesStore()

// ── Context section title ──

const sectionTitle = computed(() => {
  switch (route.path) {
    case '/': return 'Overview'
    case '/strategies': return 'Strategies'
    case '/backtest': return 'Backtest Results'
    case '/terminal': return 'Log Filters'
    case '/journal': return 'Journal Filters'
    case '/charts': return 'Chart Options'
    case '/exchange': return 'Exchanges'
    case '/settings': return 'Context'
    default: return 'Context'
  }
})

// ── Terminal log filter state ──

const logFilters = reactive({
  info: true,
  trade: true,
  warn: true,
  error: true,
})

const terminalStrategyFilter = ref<string>('')

// ── Journal filter state ──

const journalDateFrom = ref('')
const journalDateTo = ref('')
const journalStrategy = ref<string>('')
const journalPair = ref('')
const journalSide = ref<string>('')

// ── Charts state ──

const chartTimeframes = ['1h', '4h', '1d', '1w', '1M', 'All'] as const
const selectedTimeframe = ref<string>('1d')
const showVolume = ref(true)
const showMA = ref(false)
const showBollingerBands = ref(false)
</script>

<template>
  <aside class="right-sidebar">
    <div class="right-content">
      <div class="section-title">{{ sectionTitle }}</div>

      <!-- Dashboard context -->
      <template v-if="route.path === '/'">
        <div class="context-section">
          <div class="context-sublabel">Quick Balances</div>
          <div class="context-row">
            <span class="row-label">Equity</span>
            <span class="row-value mono">${{ bot.equity.toLocaleString() }}</span>
          </div>
          <div class="context-row">
            <span class="row-label">Today PnL</span>
            <span
              class="row-value mono"
              :class="bot.pnl.today >= 0 ? 'text-success' : 'text-error'"
            >
              {{ bot.pnl.today >= 0 ? '+' : '' }}{{ bot.pnl.today.toFixed(2) }}%
            </span>
          </div>
          <div class="context-row">
            <span class="row-label">Open Positions</span>
            <span class="row-value mono">{{ bot.openPositions.length }}</span>
          </div>
        </div>
        <div class="context-section">
          <div class="context-sublabel">Active Pair</div>
          <div class="context-value">{{ bot.activePair ?? 'None' }}</div>
        </div>
      </template>

      <!-- Strategies context -->
      <template v-else-if="route.path === '/strategies'">
        <div v-if="strategies.strategies.length === 0" class="context-empty">
          No strategies saved
        </div>
        <div v-else class="strategy-list">
          <button
            v-for="strat in strategies.strategies"
            :key="strat.id"
            class="strategy-item"
            :class="{ active: strategies.activeStrategyId === strat.id }"
            @click="strategies.select(strat.id)"
          >
            <span class="strategy-name">{{ strat.name }}</span>
          </button>
        </div>
      </template>

      <!-- Backtest context -->
      <template v-else-if="route.path === '/backtest'">
        <div class="context-empty">
          No saved results
        </div>
      </template>

      <!-- Terminal context -->
      <template v-else-if="route.path === '/terminal'">
        <div class="context-section">
          <div class="context-sublabel">Log Level</div>
          <div class="filter-checks">
            <label class="check-label">
              <input v-model="logFilters.info" type="checkbox" />
              <span>INFO</span>
            </label>
            <label class="check-label">
              <input v-model="logFilters.trade" type="checkbox" />
              <span>TRADE</span>
            </label>
            <label class="check-label">
              <input v-model="logFilters.warn" type="checkbox" />
              <span>WARN</span>
            </label>
            <label class="check-label">
              <input v-model="logFilters.error" type="checkbox" />
              <span>ERROR</span>
            </label>
          </div>
        </div>
        <div class="context-section">
          <div class="context-sublabel">Strategy Filter</div>
          <select v-model="terminalStrategyFilter" class="input input-sm">
            <option value="">All strategies</option>
            <option
              v-for="strat in strategies.strategies"
              :key="strat.id"
              :value="strat.id"
            >
              {{ strat.name }}
            </option>
          </select>
        </div>
      </template>

      <!-- Journal context -->
      <template v-else-if="route.path === '/journal'">
        <div class="context-section">
          <div class="context-sublabel">Date Range</div>
          <div class="filter-row">
            <input
              v-model="journalDateFrom"
              type="date"
              class="input input-sm"
              placeholder="From"
            />
            <input
              v-model="journalDateTo"
              type="date"
              class="input input-sm"
              placeholder="To"
            />
          </div>
        </div>
        <div class="context-section">
          <div class="context-sublabel">Strategy</div>
          <select v-model="journalStrategy" class="input input-sm">
            <option value="">All strategies</option>
            <option
              v-for="strat in strategies.strategies"
              :key="strat.id"
              :value="strat.id"
            >
              {{ strat.name }}
            </option>
          </select>
        </div>
        <div class="context-section">
          <div class="context-sublabel">Pair</div>
          <input
            v-model="journalPair"
            type="text"
            class="input input-sm"
            placeholder="e.g. BTC/USDT"
          />
        </div>
        <div class="context-section">
          <div class="context-sublabel">Side</div>
          <select v-model="journalSide" class="input input-sm">
            <option value="">All</option>
            <option value="long">Long</option>
            <option value="short">Short</option>
          </select>
        </div>
      </template>

      <!-- Charts context -->
      <template v-else-if="route.path === '/charts'">
        <div class="context-section">
          <div class="context-sublabel">Timeframe</div>
          <div class="timeframe-grid">
            <button
              v-for="tf in chartTimeframes"
              :key="tf"
              class="btn btn-sm tf-btn"
              :class="{ active: selectedTimeframe === tf }"
              @click="selectedTimeframe = tf"
            >
              {{ tf }}
            </button>
          </div>
        </div>
        <div class="context-section">
          <div class="context-sublabel">Overlays</div>
          <div class="filter-checks">
            <label class="check-label">
              <input v-model="showVolume" type="checkbox" />
              <span>Volume</span>
            </label>
            <label class="check-label">
              <input v-model="showMA" type="checkbox" />
              <span>Moving Average</span>
            </label>
            <label class="check-label">
              <input v-model="showBollingerBands" type="checkbox" />
              <span>Bollinger Bands</span>
            </label>
          </div>
        </div>
      </template>

      <!-- Exchange context -->
      <template v-else-if="route.path === '/exchange'">
        <div class="context-empty">
          No exchanges configured
        </div>
      </template>

    </div>
  </aside>
</template>

<style scoped>
.right-sidebar {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.right-content {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding-top: 4px;
}

.section-title {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--qa-text-muted);
  padding: 16px 16px 8px;
}

.context-section {
  padding: 0 16px;
  margin-bottom: 16px;
}

.context-sublabel {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--qa-text-muted);
  margin-bottom: 8px;
}

.context-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 5px 0;
  font-size: 12px;
}

.context-row:hover {
  background: var(--qa-bg-hover);
  border-radius: 4px;
}

.row-label {
  color: var(--qa-text-secondary);
}

.row-value {
  color: var(--qa-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 0;
  max-width: 50%;
  text-align: right;
}

.context-value {
  font-size: 12px;
  color: var(--qa-text);
}

.context-empty {
  font-size: 12px;
  color: var(--qa-text-muted);
  text-align: center;
  padding: 24px 16px;
}

.mono {
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', 'Fira Code', Menlo, Consolas, monospace;
}

/* ── Strategy list ── */

.strategy-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: 0 8px;
}

.strategy-item {
  display: flex;
  align-items: center;
  padding: 7px 10px;
  border-radius: 6px;
  background: none;
  border: none;
  cursor: pointer;
  text-align: left;
  color: var(--qa-text-secondary);
  font-size: 12px;
  transition: background var(--qa-transition), color var(--qa-transition);
}

.strategy-item:hover {
  background: var(--qa-bg-hover);
  color: var(--qa-text);
}

.strategy-item.active {
  background: var(--qa-bg-hover);
  color: var(--qa-text);
}

.strategy-name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ── Filters ── */

.filter-checks {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.check-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--qa-text-secondary);
  cursor: pointer;
  transition: color var(--qa-transition);
}

.check-label:hover {
  color: var(--qa-text);
}

.check-label input[type="checkbox"] {
  accent-color: var(--qa-accent);
  width: 13px;
  height: 13px;
}

.filter-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.input-sm {
  padding: 5px 10px;
  font-size: 11px;
  width: 100%;
  max-width: 100%;
  box-sizing: border-box;
}

/* ── Timeframe grid ── */

.timeframe-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 4px;
}

.tf-btn {
  font-size: 11px;
  padding: 5px 4px;
}

.tf-btn.active {
  background: var(--qa-accent);
  color: var(--qa-bg);
  border-color: var(--qa-accent);
}
</style>
