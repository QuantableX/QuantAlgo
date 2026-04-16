<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { useStrategiesStore } from '~/stores/strategies'
import { useExchangeStore } from '~/stores/exchange'
import { useAppStore } from '~/stores/app'
import { useBotStore } from '~/stores/bot'
import type {
  TradingMode,
  PreflightResult,
  PreflightCheck,
  DeployConfig,
} from '~/types'

// ── Props & Emits ──

const props = withDefaults(defineProps<{
  visible: boolean
  strategyId: string | null
  isDirty: boolean
  saveError?: string | null
}>(), {
  saveError: null,
})

const emit = defineEmits<{
  close: []
  'save-first': []
  started: []
}>()

// ── Stores ──

const strategiesStore = useStrategiesStore()
const exchangeStore = useExchangeStore()
const appStore = useAppStore()
const botStore = useBotStore()

// ── Local State ──

const selectedExchangeId = ref<string>('')
const selectedPair = ref<string>('')
const tradingMode = ref<TradingMode>('paper')
const timeframe = ref<string>('1h')
const initialBalance = ref<number>(10000)

const isLoadingPairs = ref(false)
const loadPairsError = ref<string | null>(null)

const preflightResult = ref<PreflightResult | null>(null)
const isRunningPreflight = ref(false)
const preflightError = ref<string | null>(null)
let preflightRequestId = 0

const isStarting = ref(false)
const startError = ref<string | null>(null)

// ── Computed ──

const strategy = computed(() => {
  if (!props.strategyId) return null
  return strategiesStore.strategies.find((s) => s.id === props.strategyId) ?? null
})

const exchanges = computed(() => exchangeStore.exchanges)
const pairs = computed(() => exchangeStore.pairs)
const settings = computed(() => appStore.settings)

const hasExchanges = computed(() => exchanges.value.length > 0)

const canRunPreflight = computed(() =>
  !!props.strategyId && !!selectedExchangeId.value && !!selectedPair.value,
)

const canStart = computed(() =>
  canRunPreflight.value
  && preflightResult.value !== null
  && preflightResult.value.can_start
  && !isStarting.value
  && !props.isDirty,
)

const preflightHasFatal = computed(() =>
  preflightResult.value !== null && !preflightResult.value.can_start,
)

const timeframeOptions = [
  { value: '1m', label: '1m' },
  { value: '5m', label: '5m' },
  { value: '15m', label: '15m' },
  { value: '1h', label: '1h' },
  { value: '4h', label: '4h' },
  { value: '1d', label: '1d' },
]

const deployConfig = computed<DeployConfig | null>(() => {
  if (!props.strategyId || !selectedExchangeId.value || !selectedPair.value) return null
  return {
    strategy_id: props.strategyId,
    exchange_id: selectedExchangeId.value,
    pair: selectedPair.value,
    trading_mode: tradingMode.value,
    timeframe: timeframe.value,
    initial_balance: initialBalance.value,
    risk_per_trade: settings.value.risk_per_trade,
    max_positions: settings.value.max_concurrent_positions,
    slippage: settings.value.slippage_tolerance,
    fee: settings.value.paper_fee_pct,
  }
})

const preflightKey = computed(() => (
  deployConfig.value ? JSON.stringify(deployConfig.value) : ''
))

// ── Reset State ──

function resetForm() {
  selectedPair.value = ''
  tradingMode.value = 'paper'
  timeframe.value = settings.value.default_timeframe || '1h'
  initialBalance.value = 10000
  preflightResult.value = null
  preflightError.value = null
  preflightRequestId += 1
  isRunningPreflight.value = false
  isStarting.value = false
  startError.value = null
  loadPairsError.value = null
  isLoadingPairs.value = false

  // Auto-select first exchange
  const first = exchanges.value[0]
  selectedExchangeId.value = first?.id ?? ''
}

// ── Watchers ──

// Reset when modal opens
watch(() => props.visible, (val) => {
  if (val) {
    resetForm()
    if (selectedExchangeId.value) {
      loadPairsForExchange(selectedExchangeId.value)
    }
  }
})

// Reload pairs when exchange changes
watch(selectedExchangeId, (id) => {
  if (id) {
    loadPairsForExchange(id)
  } else {
    selectedPair.value = ''
  }
  // Invalidate preflight when exchange changes
  preflightResult.value = null
  preflightError.value = null
  preflightRequestId += 1
})

// Auto-run preflight when the full deploy config changes
watchDebounced(preflightKey, () => {
  if (canRunPreflight.value) {
    runPreflight()
  } else {
    preflightResult.value = null
    preflightError.value = null
    preflightRequestId += 1
  }
}, { debounce: 500 })

// ── Actions ──

async function loadPairsForExchange(exchangeId: string) {
  isLoadingPairs.value = true
  loadPairsError.value = null
  selectedPair.value = ''

  try {
    const loaded = await exchangeStore.loadPairs(exchangeId)

    // Default to settings pair if available
    const defaultPair = settings.value.default_pair
    if (defaultPair && loaded.includes(defaultPair)) {
      selectedPair.value = defaultPair
    } else if (loaded.length > 0) {
      selectedPair.value = loaded[0] ?? ''
    }
  } catch (err) {
    loadPairsError.value = String(err)
  } finally {
    isLoadingPairs.value = false
  }
}

async function runPreflight() {
  if (!props.strategyId || !selectedExchangeId.value || !selectedPair.value || !deployConfig.value) return

  const requestId = ++preflightRequestId
  isRunningPreflight.value = true
  preflightError.value = null
  preflightResult.value = null

  try {
    const result = await invoke<PreflightResult>('validate_bot_deploy', {
      strategyId: props.strategyId,
      exchangeId: selectedExchangeId.value,
      pair: selectedPair.value,
      tradingMode: tradingMode.value,
      config: deployConfig.value,
    })
    if (requestId === preflightRequestId) {
      preflightResult.value = result
    }
  } catch (err) {
    if (requestId === preflightRequestId) {
      preflightError.value = String(err)
    }
  } finally {
    if (requestId === preflightRequestId) {
      isRunningPreflight.value = false
    }
  }
}

async function handleStart() {
  if (!canStart.value || !props.strategyId) return
  if (props.isDirty) {
    startError.value = 'Save the strategy before starting.'
    return
  }
  const config = deployConfig.value
  if (!config) return

  isStarting.value = true
  startError.value = null

  try {
    await botStore.start(
      props.strategyId,
      selectedExchangeId.value,
      selectedPair.value,
      config as unknown as Record<string, unknown>,
      tradingMode.value,
    )
    emit('started')
    emit('close')
  } catch (err) {
    startError.value = String(err)
  } finally {
    isStarting.value = false
  }
}

function close() {
  if (!isStarting.value) {
    emit('close')
  }
}

function onOverlayClick(e: MouseEvent) {
  if (e.target === e.currentTarget) {
    close()
  }
}

function statusIcon(status: PreflightCheck['status']): string {
  switch (status) {
    case 'ok': return '\u2713'
    case 'warn': return '\u26A0'
    case 'error': return '\u2717'
  }
}

// ── Keyboard ──

onMounted(() => {
  const handler = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && props.visible) close()
  }
  window.addEventListener('keydown', handler)
  onUnmounted(() => window.removeEventListener('keydown', handler))
})
</script>

<template>
  <Teleport to="body">
    <Transition name="deploy-modal">
      <div
        v-if="visible"
        class="deploy-overlay"
        @click="onOverlayClick"
        @contextmenu.prevent
      >
        <div class="deploy-panel" @click.stop>
          <!-- Header -->
          <div class="deploy-header">
            <h2 class="deploy-title">Deploy Strategy</h2>
            <button class="close-btn" aria-label="Close" @click="close">&#10005;</button>
          </div>

          <div class="deploy-body">
            <!-- Strategy -->
            <div class="section">
              <div class="section-label">Strategy</div>
              <div class="strategy-name" v-if="strategy">
                {{ strategy.name }}
              </div>
              <div class="empty-msg" v-else>
                No strategy selected
              </div>

              <!-- Dirty warning -->
              <div v-if="isDirty" class="dirty-bar">
                <span class="dirty-icon">&#9888;</span>
                <span class="dirty-text">Strategy has unsaved changes</span>
                <button class="btn btn-sm dirty-save-btn" @click="emit('save-first')">
                  Save First
                </button>
              </div>
              <div v-if="saveError" class="error-msg">
                {{ saveError }}
              </div>
            </div>

            <!-- Exchange -->
            <div class="section">
              <div class="section-label">Exchange</div>
              <template v-if="hasExchanges">
                <select
                  v-model="selectedExchangeId"
                  class="deploy-select"
                >
                  <option value="" disabled>Select exchange</option>
                  <option
                    v-for="ex in exchanges"
                    :key="ex.id"
                    :value="ex.id"
                  >
                    {{ ex.name }} ({{ ex.provider }})
                  </option>
                </select>
              </template>
              <div v-else class="empty-msg">
                No exchanges configured &mdash; add one in Exchange page
              </div>
            </div>

            <!-- Pair -->
            <div class="section">
              <div class="section-label">Trading Pair</div>
              <template v-if="isLoadingPairs">
                <div class="loading-msg">Loading pairs...</div>
              </template>
              <template v-else-if="loadPairsError">
                <div class="error-msg">{{ loadPairsError }}</div>
                <div class="empty-msg">
                  Paper deploy requires public exchange pair metadata and market data for the selected provider.
                </div>
              </template>
              <template v-else-if="pairs.length > 0">
                <select
                  v-model="selectedPair"
                  class="deploy-select"
                >
                  <option value="" disabled>Select pair</option>
                  <option
                    v-for="p in pairs"
                    :key="p"
                    :value="p"
                  >
                    {{ p }}
                  </option>
                </select>
              </template>
              <div v-else-if="selectedExchangeId" class="empty-msg">
                No pairs available. Paper deploy requires public exchange pair metadata for this provider.
              </div>
              <div v-else class="empty-msg">
                Select an exchange first
              </div>
            </div>

            <!-- Trading Mode -->
            <div class="section">
              <div class="section-label">Trading Mode</div>
              <div class="mode-buttons">
                <button
                  class="mode-btn"
                  :class="{ active: tradingMode === 'paper' }"
                  @click="tradingMode = 'paper'"
                >
                  Paper Trading
                </button>
                <div class="mode-btn-wrapper">
                  <button
                    class="mode-btn"
                    :class="{ disabled: true }"
                    disabled
                    title="Not yet implemented"
                  >
                    Live Trading
                  </button>
                  <span class="mode-tooltip">Not yet implemented</span>
                </div>
              </div>
            </div>

            <!-- Paper Config -->
            <div v-if="tradingMode === 'paper'" class="section">
              <div class="section-label">Paper Configuration</div>
              <div class="config-grid">
                <div class="config-row">
                  <span class="config-label">Timeframe</span>
                  <select
                    v-model="timeframe"
                    class="deploy-select deploy-select--sm"
                  >
                    <option
                      v-for="tf in timeframeOptions"
                      :key="tf.value"
                      :value="tf.value"
                    >
                      {{ tf.label }}
                    </option>
                  </select>
                </div>
                <div class="config-row">
                  <span class="config-label">Initial Balance</span>
                  <div class="balance-input-wrap">
                    <span class="balance-prefix">$</span>
                    <input
                      v-model.number="initialBalance"
                      type="number"
                      class="deploy-input deploy-input--balance"
                      min="100"
                      step="100"
                    />
                  </div>
                </div>
              </div>
            </div>

            <!-- Risk Settings -->
            <div class="section">
              <div class="section-label">Risk Settings</div>
              <div class="risk-grid">
                <div class="risk-item">
                  <span class="risk-label">Risk per trade</span>
                  <span class="risk-value mono">{{ settings.risk_per_trade }}%</span>
                </div>
                <div class="risk-item">
                  <span class="risk-label">Max positions</span>
                  <span class="risk-value mono">{{ settings.max_concurrent_positions }}</span>
                </div>
                <div class="risk-item">
                  <span class="risk-label">Slippage</span>
                  <span class="risk-value mono">{{ settings.slippage_tolerance }}%</span>
                </div>
                <div class="risk-item">
                  <span class="risk-label">Paper fee</span>
                  <span class="risk-value mono">{{ settings.paper_fee_pct }}%</span>
                </div>
              </div>
            </div>

            <!-- Preflight Checklist -->
            <div class="section">
              <div class="section-label-row">
                <span class="section-label" style="margin-bottom: 0;">Preflight Checks</span>
                <button
                  class="btn btn-sm preflight-btn"
                  :disabled="!canRunPreflight || isRunningPreflight"
                  @click="runPreflight"
                >
                  {{ isRunningPreflight ? 'Checking...' : 'Run Preflight' }}
                </button>
              </div>

              <div v-if="isRunningPreflight" class="loading-msg">
                Running preflight checks...
              </div>

              <div v-else-if="preflightError" class="error-msg">
                {{ preflightError }}
              </div>

              <div v-else-if="preflightResult" class="checklist">
                <div
                  v-for="check in preflightResult.checks"
                  :key="check.id"
                  class="check-item"
                  :class="'check-' + check.status"
                >
                  <span class="check-icon">{{ statusIcon(check.status) }}</span>
                  <div class="check-body">
                    <span class="check-label">{{ check.label }}</span>
                    <span class="check-message">{{ check.message }}</span>
                  </div>
                </div>

                <div v-if="preflightHasFatal" class="preflight-blocked">
                  Preflight failed &mdash; resolve errors before starting
                </div>
              </div>

              <div v-else class="empty-msg">
                Preflight runs automatically when all fields are selected
              </div>
            </div>

            <!-- Start Error -->
            <div v-if="startError" class="error-msg" style="margin-top: -8px;">
              {{ startError }}
            </div>
          </div>

          <!-- Footer -->
          <div class="deploy-footer">
            <button class="btn" @click="close" :disabled="isStarting">
              Cancel
            </button>
            <button
              class="btn btn-success start-btn"
              :disabled="!canStart"
              @click="handleStart"
            >
              <template v-if="isStarting">
                <span class="spinner" />
                Starting...
              </template>
              <template v-else>
                <span class="start-icon">&#9654;</span>
                Start Bot
              </template>
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style>
/* ── Deploy Modal (unscoped due to Teleport to body) ── */

.deploy-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(2px);
}

/* ── Panel ── */

.deploy-panel {
  width: 560px;
  max-width: 95vw;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  background: var(--qa-bg-sidebar);
  border: 1px solid var(--qa-border);
  border-radius: var(--qa-radius-lg);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4);
}

/* ── Header ── */

.deploy-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid var(--qa-border);
  flex-shrink: 0;
}

.deploy-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--qa-text);
  margin: 0;
}

.deploy-overlay .close-btn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--qa-text-muted);
  font-size: 12px;
  cursor: pointer;
  transition: color var(--qa-transition);
}

.deploy-overlay .close-btn:hover {
  color: var(--qa-text);
}

/* ── Body ── */

.deploy-body {
  padding: 20px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* ── Footer ── */

.deploy-footer {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 8px;
  padding: 12px 20px;
  border-top: 1px solid var(--qa-border);
  flex-shrink: 0;
}

/* ── Sections (scoped under deploy-overlay) ── */

.deploy-overlay .section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--qa-border-subtle);
}

.deploy-overlay .section:last-child {
  border-bottom: none;
  padding-bottom: 0;
}

.deploy-overlay .section-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-weight: 500;
  color: var(--qa-text-muted);
  margin-bottom: 2px;
}

.deploy-overlay .section-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}

/* ── Strategy Name ── */

.deploy-overlay .strategy-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--qa-text);
  padding: 8px 12px;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 8px;
}

/* ── Dirty Warning ── */

.deploy-overlay .dirty-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: color-mix(in srgb, var(--qa-warning) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--qa-warning) 30%, transparent);
  border-radius: 8px;
}

.deploy-overlay .dirty-icon {
  color: var(--qa-warning);
  font-size: 14px;
  flex-shrink: 0;
}

.deploy-overlay .dirty-text {
  font-size: 12px;
  color: var(--qa-warning);
  flex: 1;
}

.deploy-overlay .dirty-save-btn {
  background: color-mix(in srgb, var(--qa-warning) 20%, transparent);
  border-color: var(--qa-warning);
  color: var(--qa-warning);
  white-space: nowrap;
  flex-shrink: 0;
}

.deploy-overlay .dirty-save-btn:hover {
  background: color-mix(in srgb, var(--qa-warning) 30%, transparent);
}

/* ── Selects ── */

.deploy-select {
  width: 100%;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 12px;
  outline: none;
  cursor: pointer;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-input, var(--qa-bg-hover));
  color: var(--qa-text);
  transition: border-color var(--qa-transition);
}

.deploy-select:focus {
  border-color: var(--qa-accent);
}

.deploy-select option {
  background: var(--qa-bg-sidebar);
  color: var(--qa-text);
}

.deploy-select--sm {
  width: auto;
  min-width: 80px;
  padding: 6px 10px;
}

/* ── Inputs ── */

.deploy-input {
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 12px;
  outline: none;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-input, var(--qa-bg-hover));
  color: var(--qa-text);
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', Menlo, Consolas, monospace;
  transition: border-color var(--qa-transition);
}

.deploy-input:focus {
  border-color: var(--qa-accent);
}

.deploy-input--balance {
  width: 100px;
  text-align: right;
}

.deploy-overlay .balance-input-wrap {
  display: flex;
  align-items: center;
  gap: 4px;
}

.deploy-overlay .balance-prefix {
  font-size: 12px;
  color: var(--qa-text-muted);
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', Menlo, Consolas, monospace;
}

/* ── Mode Buttons ── */

.deploy-overlay .mode-buttons {
  display: flex;
  gap: 6px;
}

.deploy-overlay .mode-btn {
  flex: 1;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-hover);
  color: var(--qa-text);
  cursor: pointer;
  transition: all var(--qa-transition);
}

.deploy-overlay .mode-btn.active {
  background: var(--qa-accent);
  color: var(--qa-bg);
  border-color: var(--qa-accent);
}

.deploy-overlay .mode-btn.disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.deploy-overlay .mode-btn-wrapper {
  flex: 1;
  position: relative;
}

.deploy-overlay .mode-btn-wrapper .mode-btn {
  width: 100%;
}

.deploy-overlay .mode-tooltip {
  display: none;
  position: absolute;
  bottom: calc(100% + 6px);
  left: 50%;
  transform: translateX(-50%);
  padding: 4px 8px;
  background: var(--qa-bg-card);
  border: 1px solid var(--qa-border);
  border-radius: 4px;
  font-size: 11px;
  color: var(--qa-text-secondary);
  white-space: nowrap;
  pointer-events: none;
  z-index: 10;
}

.deploy-overlay .mode-btn-wrapper:hover .mode-tooltip {
  display: block;
}

/* ── Paper Config ── */

.deploy-overlay .config-grid {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.deploy-overlay .config-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 8px;
}

.deploy-overlay .config-label {
  font-size: 12px;
  color: var(--qa-text);
}

/* ── Risk Settings ── */

.deploy-overlay .risk-grid {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.deploy-overlay .risk-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 8px;
}

.deploy-overlay .risk-label {
  font-size: 12px;
  color: var(--qa-text-secondary);
}

.deploy-overlay .risk-value {
  font-size: 12px;
  font-weight: 500;
  color: var(--qa-text);
}

/* ── Mono ── */

.mono {
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', Menlo, Consolas, monospace;
}

/* ── Preflight ── */

.deploy-overlay .preflight-btn {
  flex-shrink: 0;
}

.deploy-overlay .checklist {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.deploy-overlay .check-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 12px;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 8px;
}

.deploy-overlay .check-icon {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  line-height: 1;
  margin-top: 1px;
}

.deploy-overlay .check-ok .check-icon {
  color: var(--qa-success);
}

.deploy-overlay .check-warn .check-icon {
  color: var(--qa-warning);
}

.deploy-overlay .check-error .check-icon {
  color: var(--qa-error);
}

.deploy-overlay .check-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.deploy-overlay .check-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--qa-text);
}

.deploy-overlay .check-message {
  font-size: 11px;
  color: var(--qa-text-muted);
  line-height: 1.4;
}

.deploy-overlay .preflight-blocked {
  font-size: 11px;
  color: var(--qa-error);
  padding: 6px 12px;
  background: color-mix(in srgb, var(--qa-error) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--qa-error) 25%, transparent);
  border-radius: 6px;
  margin-top: 4px;
}

/* ── Messages ── */

.deploy-overlay .empty-msg {
  font-size: 12px;
  color: var(--qa-text-muted);
  padding: 8px 12px;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 8px;
}

.deploy-overlay .loading-msg {
  font-size: 12px;
  color: var(--qa-text-secondary);
  padding: 8px 12px;
}

.deploy-overlay .error-msg {
  font-size: 12px;
  color: var(--qa-error);
  padding: 8px 12px;
  background: color-mix(in srgb, var(--qa-error) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--qa-error) 25%, transparent);
  border-radius: 8px;
}

/* ── Start Button ── */

.deploy-overlay .start-btn {
  display: flex;
  align-items: center;
  gap: 6px;
}

.deploy-overlay .start-icon {
  font-size: 10px;
  line-height: 1;
}

/* ── Spinner ── */

.deploy-overlay .spinner {
  display: inline-block;
  width: 12px;
  height: 12px;
  border: 2px solid currentColor;
  border-right-color: transparent;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* ── Transition ── */

.deploy-modal-enter-active,
.deploy-modal-leave-active {
  transition: opacity 150ms ease;
}

.deploy-modal-enter-active .deploy-panel,
.deploy-modal-leave-active .deploy-panel {
  transition: transform 150ms ease, opacity 150ms ease;
}

.deploy-modal-enter-from,
.deploy-modal-leave-to {
  opacity: 0;
}

.deploy-modal-enter-from .deploy-panel {
  transform: scale(0.96) translateY(8px);
  opacity: 0;
}

.deploy-modal-leave-to .deploy-panel {
  transform: scale(0.96) translateY(8px);
  opacity: 0;
}
</style>
