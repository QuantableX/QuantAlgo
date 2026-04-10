<script setup lang="ts">
import { useBacktestStore } from '~/stores/backtest'
import { useStrategiesStore } from '~/stores/strategies'
import type { BacktestConfig, BacktestResult } from '~/types'

const route = useRoute()
const backtestStore = useBacktestStore()
const strategiesStore = useStrategiesStore()

// UI state
const showConfig = ref(true)
const saveName = ref('')
const showSavePrompt = ref(false)
const isSaving = ref(false)

// Pre-selected strategy from query param
const preSelectedStrategyId = computed(() => {
  return (route.query.strategy as string) ?? null
})

// Comparison mode
const comparisonResults = computed(() => backtestStore.comparisonResults)
const isComparing = computed(() => comparisonResults.value.length > 1)

async function handleRun(config: BacktestConfig) {
  showConfig.value = false
  try {
    await backtestStore.run(config)
  } catch (err) {
    console.error('Backtest failed:', err)
    showConfig.value = true
  }
}

function handleReconfigure() {
  showConfig.value = true
}

function handleSavePrompt() {
  if (!backtestStore.currentResult) return
  saveName.value = ''
  showSavePrompt.value = true
}

async function handleSave() {
  if (!backtestStore.currentResult || !saveName.value.trim()) return
  isSaving.value = true
  try {
    await backtestStore.save(backtestStore.currentResult, saveName.value.trim())
    showSavePrompt.value = false
    saveName.value = ''
  } catch (err) {
    console.error('Failed to save backtest:', err)
  } finally {
    isSaving.value = false
  }
}

function cancelSave() {
  showSavePrompt.value = false
  saveName.value = ''
}

async function handleLoadSaved(id: string) {
  try {
    await backtestStore.get(id)
    showConfig.value = false
  } catch (err) {
    console.error('Failed to load backtest:', err)
  }
}

async function handleDeleteSaved(id: string) {
  try {
    await backtestStore.delete(id)
  } catch (err) {
    console.error('Failed to delete backtest:', err)
  }
}

// Watch for result completion to switch view
watch(() => backtestStore.currentResult, (result) => {
  if (result && !backtestStore.isRunning) {
    showConfig.value = false
  }
})

onMounted(async () => {
  await Promise.all([
    backtestStore.load(),
    backtestStore.init(),
    strategiesStore.load(),
  ])
})
</script>

<template>
  <div class="backtest-page">
    <div class="backtest-main">
      <!-- Configuration Section -->
      <div v-if="showConfig && !backtestStore.isRunning" class="config-section">
        <BacktestConfig
          :pre-selected-strategy-id="preSelectedStrategyId"
          @run="handleRun"
        />
      </div>

      <!-- Progress Bar -->
      <div v-if="backtestStore.isRunning" class="progress-section">
        <div class="progress-card card">
          <h3 class="progress-title">Running Backtest</h3>
          <div class="progress-bar-track">
            <div
              class="progress-bar-fill"
              :style="{ width: `${backtestStore.progress.pct}%` }"
            />
          </div>
          <div class="progress-info">
            <span class="progress-pct">{{ Math.round(backtestStore.progress.pct) }}%</span>
            <span class="progress-message text-muted">{{ backtestStore.progress.message }}</span>
          </div>
        </div>
      </div>

      <!-- Results Section -->
      <div v-if="!showConfig && !backtestStore.isRunning && backtestStore.currentResult" class="results-section">
        <div class="results-toolbar">
          <button class="btn btn-sm" @click="handleReconfigure">
            New Backtest
          </button>
          <button class="btn btn-sm btn-primary" @click="handleSavePrompt">
            Save Result
          </button>
        </div>

        <!-- Save prompt overlay -->
        <div v-if="showSavePrompt" class="save-prompt card">
          <h4 class="save-prompt__title">Save Backtest Result</h4>
          <div class="save-prompt__field">
            <label class="label" for="save-name">Name</label>
            <input
              id="save-name"
              v-model="saveName"
              class="input"
              type="text"
              placeholder="e.g., BTC RSI Strategy - April 2026"
              @keydown.enter="handleSave"
            />
          </div>
          <div class="save-prompt__actions">
            <button class="btn btn-sm" @click="cancelSave">Cancel</button>
            <button
              class="btn btn-sm btn-primary"
              :disabled="!saveName.trim() || isSaving"
              @click="handleSave"
            >
              {{ isSaving ? 'Saving...' : 'Save' }}
            </button>
          </div>
        </div>

        <BacktestResults :result="backtestStore.currentResult" />

        <!-- Comparison view -->
        <div v-if="isComparing" class="comparison-section">
          <BacktestComparison :results="comparisonResults" />
        </div>
      </div>

      <!-- Empty state when no config and no result -->
      <div
        v-if="!showConfig && !backtestStore.isRunning && !backtestStore.currentResult"
        class="empty-state"
      >
        <p class="text-muted">No backtest result loaded.</p>
        <button class="btn" @click="showConfig = true">Configure Backtest</button>
      </div>
    </div>

    <!-- Saved backtests sidebar -->
    <aside v-if="backtestStore.backtests.length" class="backtest-sidebar">
      <h3 class="sidebar-title">Saved Backtests</h3>
      <div class="saved-list">
        <div
          v-for="bt in backtestStore.backtests"
          :key="bt.id"
          class="saved-item"
          :class="{ 'saved-item--active': bt.id === backtestStore.activeBacktestId }"
          @click="handleLoadSaved(bt.id)"
        >
          <div class="saved-item__name">{{ bt.name }}</div>
          <div class="saved-item__meta text-muted">
            {{ bt.created_at ? new Date(bt.created_at).toLocaleDateString() : '' }}
          </div>
          <button
            class="saved-item__delete btn btn-sm"
            @click.stop="handleDeleteSaved(bt.id)"
          >
            Delete
          </button>
        </div>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.backtest-page {
  display: flex;
  height: 100%;
  overflow: hidden;
}

.backtest-main {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 20px;
}

/* Config section */
.config-section {
  max-width: 720px;
}

/* Progress */
.progress-section {
  max-width: 600px;
  margin: 80px auto 0;
}

.progress-card {
  text-align: center;
}

.progress-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--qa-text);
  margin-bottom: 20px;
}

.progress-bar-track {
  width: 100%;
  height: 4px;
  background: var(--qa-bg-card);
  border: 1px solid var(--qa-border-subtle);
  border-radius: 2px;
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: var(--qa-accent);
  border-radius: 2px;
  transition: width 300ms ease;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 12px;
}

.progress-pct {
  font-size: 13px;
  font-weight: 600;
  color: var(--qa-text);
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', monospace;
}

.progress-message {
  font-size: 13px;
}

/* Results */
.results-section {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.results-toolbar {
  display: flex;
  gap: 8px;
}

/* Save prompt */
.save-prompt {
  max-width: 480px;
}

.save-prompt__title {
  font-size: 14px;
  font-weight: 600;
  color: var(--qa-text);
  margin-bottom: 12px;
}

.save-prompt__field {
  margin-bottom: 12px;
}

.save-prompt__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* Comparison */
.comparison-section {
  margin-top: 24px;
  padding-top: 24px;
  border-top: 1px solid var(--qa-border);
}

/* Empty state */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  padding: 80px 0;
}

/* Sidebar */
.backtest-sidebar {
  width: 260px;
  flex-shrink: 0;
  background: var(--qa-bg-sidebar);
  border-left: 1px solid var(--qa-border);
  overflow-y: auto;
  padding: 16px;
}

.sidebar-title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--qa-text-muted);
  margin-bottom: 12px;
}

.saved-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.saved-item {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 10px 12px;
  border-radius: var(--qa-radius);
  cursor: pointer;
  transition: background var(--qa-transition);
}

.saved-item:hover {
  background: var(--qa-bg-hover);
}

.saved-item--active {
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
}

.saved-item__name {
  flex: 1;
  font-size: 13px;
  font-weight: 500;
  color: var(--qa-text);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.saved-item__meta {
  font-size: 11px;
  width: 100%;
}

.saved-item__delete {
  margin-left: auto;
  font-size: 11px;
  padding: 2px 8px;
  opacity: 0;
  transition: opacity var(--qa-transition);
}

.saved-item:hover .saved-item__delete {
  opacity: 1;
}
</style>
