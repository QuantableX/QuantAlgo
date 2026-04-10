<script setup lang="ts">
import { useAppStore } from '~/stores/app'
import { invoke } from '@tauri-apps/api/core'
import type { AppSettings } from '~/types'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  close: []
}>()

const app = useAppStore()
const config = useRuntimeConfig()

// ── Local state synced on open ──

const fontSize = ref(app.settings.font_size)
const defaultPair = ref(app.settings.default_pair)
const defaultTimeframe = ref(app.settings.default_timeframe)
const pythonPath = ref(app.settings.python_path)
const riskPerTrade = ref(app.settings.risk_per_trade)
const maxPositions = ref(app.settings.max_concurrent_positions)
const slippageTolerance = ref(app.settings.slippage_tolerance)
const notifyTrade = ref(app.settings.notify_on_trade)
const notifyError = ref(app.settings.notify_on_error)
const notifyDailySummary = ref(app.settings.notify_on_daily_summary)

const isDetectingPython = ref(false)
const pythonDetected = ref(false)
const pythonDetectError = ref<string | null>(null)

const timeframeOptions = ['1m', '5m', '15m', '1h', '4h', '1d']

watch(() => props.visible, (open) => {
  if (open) {
    fontSize.value = app.settings.font_size
    defaultPair.value = app.settings.default_pair
    defaultTimeframe.value = app.settings.default_timeframe
    pythonPath.value = app.settings.python_path
    riskPerTrade.value = app.settings.risk_per_trade
    maxPositions.value = app.settings.max_concurrent_positions
    slippageTolerance.value = app.settings.slippage_tolerance
    notifyTrade.value = app.settings.notify_on_trade
    notifyError.value = app.settings.notify_on_error
    notifyDailySummary.value = app.settings.notify_on_daily_summary
    pythonDetected.value = false
    pythonDetectError.value = null
  }
})

// ── Actions (auto-save on change) ──

function toggleTheme() {
  const next = app.settings.theme === 'dark' ? 'light' : 'dark'
  app.saveSettings({ theme: next })
}

function setFontSize(delta: number) {
  fontSize.value = Math.min(20, Math.max(10, fontSize.value + delta))
  app.saveSettings({ font_size: fontSize.value })
}

function saveDefaultPair() {
  const trimmed = defaultPair.value.trim()
  if (trimmed && trimmed !== app.settings.default_pair) {
    app.saveSettings({ default_pair: trimmed })
  }
}

function onTimeframeChange(e: Event) {
  const val = (e.target as HTMLSelectElement).value
  defaultTimeframe.value = val
  app.saveSettings({ default_timeframe: val })
}

function saveRisk() {
  app.saveSettings({ risk_per_trade: riskPerTrade.value })
}

function saveMaxPositions() {
  app.saveSettings({ max_concurrent_positions: maxPositions.value })
}

function saveSlippage() {
  app.saveSettings({ slippage_tolerance: slippageTolerance.value })
}

function savePythonPath() {
  app.saveSettings({ python_path: pythonPath.value.trim() })
}

async function detectPython() {
  isDetectingPython.value = true
  pythonDetectError.value = null
  pythonDetected.value = false
  try {
    const path = await app.detectPython()
    pythonPath.value = path
    pythonDetected.value = true
  } catch (err) {
    pythonDetectError.value = String(err)
  } finally {
    isDetectingPython.value = false
  }
}

function toggleNotifyTrade() {
  notifyTrade.value = !notifyTrade.value
  app.saveSettings({ notify_on_trade: notifyTrade.value })
}

function toggleNotifyError() {
  notifyError.value = !notifyError.value
  app.saveSettings({ notify_on_error: notifyError.value })
}

function toggleNotifyDailySummary() {
  notifyDailySummary.value = !notifyDailySummary.value
  app.saveSettings({ notify_on_daily_summary: notifyDailySummary.value })
}

async function handleExportData() {
  try { await invoke('export_all_data') } catch (err) { console.error('Export failed:', err) }
}

async function handleImportData() {
  try { await invoke('import_data') } catch (err) { console.error('Import failed:', err) }
}

function close() {
  emit('close')
}

function onOverlayClick(e: MouseEvent) {
  if (e.target === e.currentTarget) close()
}

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
    <Transition name="modal">
      <div
        v-if="visible"
        class="modal-overlay"
        @click="onOverlayClick"
        @contextmenu.prevent
      >
        <div class="modal-panel" @click.stop>
          <!-- Header -->
          <div class="modal-header">
            <h2 class="modal-title">Settings</h2>
            <button class="close-btn" aria-label="Close" @click="close">&#10005;</button>
          </div>

          <!-- Content -->
          <div class="modal-body">
            <!-- Appearance -->
            <div class="section">
              <div class="section-label">Appearance</div>
              <div class="section-group">
                <div class="row-buttons">
                  <button
                    class="theme-btn"
                    :class="{ active: app.settings.theme === 'dark' }"
                    @click="app.settings.theme !== 'dark' && toggleTheme()"
                  >Dark</button>
                  <button
                    class="theme-btn"
                    :class="{ active: app.settings.theme === 'light' }"
                    @click="app.settings.theme !== 'light' && toggleTheme()"
                  >Light</button>
                </div>
                <div class="setting-row">
                  <span class="setting-text">Font Size</span>
                  <div class="font-size-control">
                    <button class="stepper-btn" @click="setFontSize(-1)">&minus;</button>
                    <span class="font-size-value mono">{{ fontSize }}px</span>
                    <button class="stepper-btn" @click="setFontSize(1)">+</button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Trading Defaults -->
            <div class="section">
              <div class="section-label">Trading Defaults</div>
              <div class="section-group">
                <div class="setting-row">
                  <span class="setting-text">Default Pair</span>
                  <input
                    v-model="defaultPair"
                    type="text"
                    class="setting-input"
                    placeholder="BTC/USDT"
                    @blur="saveDefaultPair"
                    @keydown.enter="saveDefaultPair"
                  />
                </div>
                <div class="setting-row">
                  <span class="setting-text">Timeframe</span>
                  <select
                    class="setting-select"
                    :value="defaultTimeframe"
                    @change="onTimeframeChange"
                  >
                    <option v-for="tf in timeframeOptions" :key="tf" :value="tf">{{ tf }}</option>
                  </select>
                </div>
                <div class="setting-row">
                  <span class="setting-text">Risk / Trade (%)</span>
                  <input
                    v-model.number="riskPerTrade"
                    type="number"
                    class="setting-input setting-input--narrow"
                    min="0.1" max="100" step="0.1"
                    @blur="saveRisk"
                    @keydown.enter="saveRisk"
                  />
                </div>
                <div class="setting-row">
                  <span class="setting-text">Max Positions</span>
                  <input
                    v-model.number="maxPositions"
                    type="number"
                    class="setting-input setting-input--narrow"
                    min="1" max="50"
                    @blur="saveMaxPositions"
                    @keydown.enter="saveMaxPositions"
                  />
                </div>
                <div class="setting-row">
                  <span class="setting-text">Slippage (%)</span>
                  <input
                    v-model.number="slippageTolerance"
                    type="number"
                    class="setting-input setting-input--narrow"
                    min="0" max="10" step="0.01"
                    @blur="saveSlippage"
                    @keydown.enter="saveSlippage"
                  />
                </div>
              </div>
            </div>

            <!-- Python -->
            <div class="section">
              <div class="section-label">Python</div>
              <div class="section-group">
                <div class="setting-row">
                  <span class="setting-text">Interpreter</span>
                  <div class="python-row">
                    <input
                      v-model="pythonPath"
                      type="text"
                      class="setting-input"
                      placeholder="/usr/bin/python3"
                      @blur="savePythonPath"
                      @keydown.enter="savePythonPath"
                    />
                    <button
                      class="detect-btn"
                      :disabled="isDetectingPython"
                      @click="detectPython"
                    >{{ isDetectingPython ? '...' : 'Detect' }}</button>
                  </div>
                </div>
                <div v-if="pythonDetected" class="detect-result detect-result--ok">
                  &#10003; Found at {{ pythonPath }}
                </div>
                <div v-if="pythonDetectError" class="detect-result detect-result--err">
                  {{ pythonDetectError }}
                </div>
              </div>
            </div>

            <!-- Notifications -->
            <div class="section">
              <div class="section-label">Notifications</div>
              <div class="section-group">
                <button class="toggle-row" @click="toggleNotifyTrade">
                  <span class="setting-text">Trade Alerts</span>
                  <span class="toggle-track" :class="{ active: notifyTrade }">
                    <span class="toggle-thumb" />
                  </span>
                </button>
                <button class="toggle-row" @click="toggleNotifyError">
                  <span class="setting-text">Error Alerts</span>
                  <span class="toggle-track" :class="{ active: notifyError }">
                    <span class="toggle-thumb" />
                  </span>
                </button>
                <button class="toggle-row" @click="toggleNotifyDailySummary">
                  <span class="setting-text">Daily Summary</span>
                  <span class="toggle-track" :class="{ active: notifyDailySummary }">
                    <span class="toggle-thumb" />
                  </span>
                </button>
              </div>
            </div>

            <!-- Data -->
            <div class="section">
              <div class="section-label">Data</div>
              <div class="section-group">
                <div class="data-actions">
                  <button class="action-btn" @click="handleExportData">Export All Data</button>
                  <button class="action-btn" @click="handleImportData">Import Data</button>
                </div>
              </div>
            </div>

            <!-- About -->
            <div class="section">
              <div class="section-label">About</div>
              <div class="about-box">
                <span class="about-version">QuantAlgo v{{ config.public.appVersion }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* ── Overlay ── */

.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 10000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(2px);
}

/* ── Panel ── */

.modal-panel {
  width: 480px;
  max-height: 80vh;
  overflow-y: auto;
  background: var(--qa-bg-sidebar);
  border: 1px solid var(--qa-border);
  border-radius: var(--qa-radius-lg);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4);
}

/* ── Header ── */

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid var(--qa-border);
}

.modal-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--qa-text);
  margin: 0;
}

.close-btn {
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

.close-btn:hover {
  color: var(--qa-text);
}

/* ── Body ── */

.modal-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

/* ── Sections ── */

.section-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-weight: 500;
  color: var(--qa-text-muted);
  margin-bottom: 8px;
}

.section-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* ── Rows ── */

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 8px;
}

.setting-text {
  font-size: 12px;
  color: var(--qa-text);
}

/* ── Theme buttons ── */

.row-buttons {
  display: flex;
  gap: 6px;
}

.theme-btn {
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

.theme-btn.active {
  background: var(--qa-accent);
  color: var(--qa-bg);
  border-color: var(--qa-accent);
}

/* ── Font size ── */

.font-size-control {
  display: flex;
  align-items: center;
  gap: 8px;
}

.stepper-btn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  font-size: 12px;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-sidebar);
  color: var(--qa-text-muted);
  cursor: pointer;
  transition: all var(--qa-transition);
}

.stepper-btn:hover {
  color: var(--qa-text);
  border-color: var(--qa-accent);
}

.font-size-value {
  font-size: 12px;
  color: var(--qa-text);
  width: 32px;
  text-align: center;
  font-weight: 500;
}

/* ── Inputs ── */

.setting-input {
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 12px;
  outline: none;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-sidebar);
  color: var(--qa-text);
  text-align: right;
  min-width: 100px;
}

.setting-input:focus {
  border-color: var(--qa-accent);
}

.setting-input--narrow {
  min-width: 60px;
  width: 60px;
}

.setting-select {
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 12px;
  outline: none;
  cursor: pointer;
  min-width: 80px;
  text-align: right;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-sidebar);
  color: var(--qa-text);
}

.setting-select option {
  background: var(--qa-bg-sidebar);
  color: var(--qa-text);
}

/* ── Python ── */

.python-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.python-row .setting-input {
  min-width: 160px;
  text-align: left;
}

.detect-btn {
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 500;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-sidebar);
  color: var(--qa-text-secondary);
  cursor: pointer;
  transition: all var(--qa-transition);
  white-space: nowrap;
}

.detect-btn:hover {
  color: var(--qa-text);
  border-color: var(--qa-accent);
}

.detect-result {
  font-size: 11px;
  padding: 0 12px;
}

.detect-result--ok {
  color: var(--qa-accent);
}

.detect-result--err {
  color: var(--qa-error);
}

/* ── Toggles ── */

.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  text-align: left;
  padding: 8px 12px;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 8px;
  cursor: pointer;
  transition: background var(--qa-transition);
}

.toggle-row:hover {
  background: var(--qa-bg-card);
}

.toggle-track {
  position: relative;
  width: 32px;
  height: 16px;
  border-radius: 8px;
  background: var(--qa-border);
  transition: background var(--qa-transition);
  flex-shrink: 0;
}

.toggle-track.active {
  background: var(--qa-accent);
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--qa-bg);
  transition: transform var(--qa-transition);
}

.toggle-track.active .toggle-thumb {
  transform: translateX(16px);
}

/* ── Data actions ── */

.data-actions {
  display: flex;
  gap: 6px;
}

.action-btn {
  flex: 1;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-sidebar);
  color: var(--qa-text);
  cursor: pointer;
  transition: all var(--qa-transition);
}

.action-btn:hover {
  background: var(--qa-bg-card);
  border-color: var(--qa-accent);
}

/* ── About ── */

.about-box {
  padding: 10px 12px;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 8px;
}

.about-version {
  font-size: 12px;
  color: var(--qa-text-muted);
}

/* ── Transition ── */

.modal-enter-active,
.modal-leave-active {
  transition: opacity 150ms ease;
}

.modal-enter-active .modal-panel,
.modal-leave-active .modal-panel {
  transition: transform 150ms ease, opacity 150ms ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .modal-panel {
  transform: scale(0.96) translateY(8px);
  opacity: 0;
}

.modal-leave-to .modal-panel {
  transform: scale(0.96) translateY(8px);
  opacity: 0;
}
</style>
