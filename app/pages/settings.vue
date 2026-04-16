<script setup lang="ts">
import { useAppStore } from '~/stores/app'
import { invoke } from '@tauri-apps/api/core'
import type { AppSettings } from '~/types'

const appStore = useAppStore()
const config = useRuntimeConfig()

// Local reactive copy of settings
const localSettings = reactive<AppSettings>({ ...appStore.settings })

// Python detection state
const isDetectingPython = ref(false)
const pythonDetected = ref(false)
const pythonDetectError = ref<string | null>(null)

// Save state
const isSaving = ref(false)
const saveMessage = ref<string | null>(null)

const timeframeOptions = ['1m', '5m', '15m', '1h', '4h', '1d']

// Sync local settings when store updates
watch(
  () => appStore.settings,
  (newSettings) => {
    Object.assign(localSettings, newSettings)
  },
  { deep: true },
)

// Auto-save with debounce
const stopWatch = watchDebounced(
  localSettings,
  async () => {
    await saveAllSettings()
  },
  { debounce: 300, deep: true },
)

async function saveAllSettings() {
  isSaving.value = true
  saveMessage.value = null
  try {
    await appStore.saveSettings({ ...localSettings })
    saveMessage.value = 'Settings saved'
    setTimeout(() => {
      saveMessage.value = null
    }, 2000)
  } catch (err) {
    console.error('Failed to save settings:', err)
    saveMessage.value = 'Failed to save'
  } finally {
    isSaving.value = false
  }
}

async function detectPython() {
  isDetectingPython.value = true
  pythonDetectError.value = null
  pythonDetected.value = false
  try {
    const path = await appStore.detectPython()
    localSettings.python_path = path
    pythonDetected.value = true
  } catch (err) {
    pythonDetectError.value = String(err)
  } finally {
    isDetectingPython.value = false
  }
}

function toggleTheme() {
  localSettings.theme = localSettings.theme === 'dark' ? 'light' : 'dark'
  appStore.applyTheme(localSettings.theme)
}

async function handleExportData() {
  try {
    await invoke('export_all_data')
  } catch (err) {
    console.error('Failed to export data:', err)
  }
}

async function handleImportData() {
  try {
    await invoke('import_data')
  } catch (err) {
    console.error('Failed to import data:', err)
  }
}

onUnmounted(() => {
  stopWatch()
})
</script>

<template>
  <div class="settings">
    <!-- Save indicator -->
    <Transition name="fade">
      <div v-if="saveMessage" class="settings__toast" :class="{ 'settings__toast--error': saveMessage === 'Failed to save' }">
        {{ saveMessage }}
      </div>
    </Transition>

    <!-- Section 1: General -->
    <section class="card settings__section">
      <h3 class="settings__section-title">General</h3>

      <div class="settings__field">
        <label class="label">Theme</label>
        <div class="settings__theme-row">
          <span class="settings__theme-value">{{ localSettings.theme === 'dark' ? 'Dark' : 'Light' }}</span>
          <button class="btn btn-sm" @click="toggleTheme">
            {{ localSettings.theme === 'dark' ? 'Switch to Light' : 'Switch to Dark' }}
          </button>
        </div>
      </div>

      <div class="settings__field">
        <label class="label" for="font-size">Font Size</label>
        <input
          id="font-size"
          v-model.number="localSettings.font_size"
          type="number"
          class="input settings__input--sm"
          min="10"
          max="20"
        />
      </div>

      <div class="settings__field">
        <label class="label" for="default-pair">Default Trading Pair</label>
        <input
          id="default-pair"
          v-model="localSettings.default_pair"
          type="text"
          class="input"
          placeholder="BTC/USDT"
        />
      </div>

      <div class="settings__field">
        <label class="label" for="default-timeframe">Default Timeframe</label>
        <select id="default-timeframe" v-model="localSettings.default_timeframe" class="input">
          <option v-for="tf in timeframeOptions" :key="tf" :value="tf">{{ tf }}</option>
        </select>
      </div>
    </section>

    <!-- Section 2: Python -->
    <section class="card settings__section">
      <h3 class="settings__section-title">Python</h3>

      <div class="settings__field">
        <label class="label" for="python-path">Python Interpreter Path</label>
        <div class="settings__path-row">
          <input
            id="python-path"
            v-model="localSettings.python_path"
            type="text"
            class="input"
            placeholder="/usr/bin/python3"
          />
          <button
            class="btn btn-sm"
            :disabled="isDetectingPython"
            @click="detectPython"
          >
            {{ isDetectingPython ? 'Detecting...' : 'Auto-detect' }}
          </button>
        </div>
        <div v-if="pythonDetected" class="settings__detect-result settings__detect-result--success">
          <span class="settings__checkmark">&#10003;</span>
          Python found at {{ localSettings.python_path }}
        </div>
        <div v-if="pythonDetectError" class="settings__detect-result settings__detect-result--error">
          {{ pythonDetectError }}
        </div>
      </div>
    </section>

    <!-- Section 3: Trading Defaults -->
    <section class="card settings__section">
      <h3 class="settings__section-title">Trading Defaults</h3>

      <div class="settings__field">
        <label class="label" for="risk-per-trade">Risk Per Trade (%)</label>
        <input
          id="risk-per-trade"
          v-model.number="localSettings.risk_per_trade"
          type="number"
          class="input settings__input--sm"
          min="0.1"
          max="10"
          step="0.1"
        />
      </div>

      <div class="settings__field">
        <label class="label" for="max-positions">Max Concurrent Positions</label>
        <input
          id="max-positions"
          v-model.number="localSettings.max_concurrent_positions"
          type="number"
          class="input settings__input--sm"
          min="1"
          max="20"
        />
      </div>

      <div class="settings__field">
        <label class="label" for="slippage">Slippage Tolerance (%)</label>
        <input
          id="slippage"
          v-model.number="localSettings.slippage_tolerance"
          type="number"
          class="input settings__input--sm"
          min="0"
          max="5"
          step="0.01"
        />
      </div>

      <div class="settings__field">
        <label class="label" for="paper-fee">Paper Fee (%)</label>
        <input
          id="paper-fee"
          v-model.number="localSettings.paper_fee_pct"
          type="number"
          class="input settings__input--sm"
          min="0"
          max="5"
          step="0.01"
        />
      </div>
    </section>

    <!-- Section 4: Notifications -->
    <section class="card settings__section">
      <h3 class="settings__section-title">Notifications</h3>

      <div class="settings__toggle-field">
        <label class="settings__toggle-label" for="notify-trade">Notify on trade fill</label>
        <label class="toggle">
          <input
            id="notify-trade"
            v-model="localSettings.notify_on_trade"
            type="checkbox"
            class="toggle__input"
          />
          <span class="toggle__slider" />
        </label>
      </div>

      <div class="settings__toggle-field">
        <label class="settings__toggle-label" for="notify-error">Notify on error</label>
        <label class="toggle">
          <input
            id="notify-error"
            v-model="localSettings.notify_on_error"
            type="checkbox"
            class="toggle__input"
          />
          <span class="toggle__slider" />
        </label>
      </div>

      <div class="settings__toggle-field">
        <label class="settings__toggle-label" for="notify-daily">Notify on daily summary</label>
        <label class="toggle">
          <input
            id="notify-daily"
            v-model="localSettings.notify_on_daily_summary"
            type="checkbox"
            class="toggle__input"
          />
          <span class="toggle__slider" />
        </label>
      </div>
    </section>

    <!-- Section 5: Data -->
    <section class="card settings__section">
      <h3 class="settings__section-title">Data</h3>

      <div class="settings__field">
        <label class="label" for="strategy-dir">Strategy Directory</label>
        <input
          id="strategy-dir"
          v-model="localSettings.strategy_dir"
          type="text"
          class="input"
          placeholder="./strategies"
        />
      </div>

      <div class="settings__field">
        <label class="label" for="backtest-dir">Backtest Data Directory</label>
        <input
          id="backtest-dir"
          v-model="localSettings.backtest_dir"
          type="text"
          class="input"
          placeholder="./backtest_data"
        />
      </div>

      <div class="settings__data-actions">
        <button class="btn" @click="handleExportData">Export All Data</button>
        <button class="btn" @click="handleImportData">Import Data</button>
      </div>
    </section>

    <!-- Section 6: About -->
    <section class="card settings__section">
      <h3 class="settings__section-title">About</h3>
      <div class="settings__about">
        <p class="settings__about-name">QuantAlgo</p>
        <p class="settings__about-desc text-muted">Crypto Trading Bot Terminal</p>
        <p class="settings__about-version text-muted">
          Version {{ config.public.appVersion }}
        </p>
        <a
          href="https://github.com/quantalgo/quantalgo"
          target="_blank"
          rel="noopener noreferrer"
          class="settings__about-link"
        >
          Documentation & Source
        </a>
      </div>
    </section>

    <!-- Manual Save Button -->
    <div class="settings__footer">
      <button
        class="btn btn-primary"
        :disabled="isSaving"
        @click="saveAllSettings"
      >
        {{ isSaving ? 'Saving...' : 'Save Settings' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.settings {
  height: 100%;
  overflow-y: auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 20px;
  position: relative;
  max-width: 720px;
}

/* Toast */
.settings__toast {
  position: fixed;
  top: 16px;
  right: 16px;
  padding: 8px 16px;
  background: var(--qa-accent);
  color: var(--qa-bg);
  font-size: 13px;
  font-weight: 500;
  border-radius: var(--qa-radius);
  z-index: 100;
}

.settings__toast--error {
  background: var(--qa-error);
  color: #fff;
}

/* Section */
.settings__section-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--qa-text);
  margin-bottom: 16px;
}

/* Fields */
.settings__field {
  margin-bottom: 16px;
}

.settings__field:last-child {
  margin-bottom: 0;
}

.settings__input--sm {
  max-width: 120px;
}

/* Theme row */
.settings__theme-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.settings__theme-value {
  font-size: 14px;
  font-weight: 500;
  color: var(--qa-text);
  min-width: 48px;
}

/* Path row */
.settings__path-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.settings__path-row .input {
  flex: 1;
}

/* Detect result */
.settings__detect-result {
  margin-top: 6px;
  font-size: 12px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.settings__detect-result--success {
  color: var(--qa-accent);
}

.settings__detect-result--error {
  color: var(--qa-error);
}

.settings__checkmark {
  font-weight: 700;
}

/* Toggle fields */
.settings__toggle-field {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 0;
  border-bottom: 1px solid var(--qa-border-subtle);
}

.settings__toggle-field:last-child {
  border-bottom: none;
}

.settings__toggle-label {
  font-size: 14px;
  color: var(--qa-text);
  cursor: pointer;
}

/* Toggle switch */
.toggle {
  position: relative;
  display: inline-block;
  width: 40px;
  height: 22px;
  flex-shrink: 0;
}

.toggle__input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle__slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: var(--qa-bg-hover);
  border: 1px solid var(--qa-border);
  border-radius: 11px;
  transition: all var(--qa-transition);
}

.toggle__slider::before {
  content: '';
  position: absolute;
  height: 16px;
  width: 16px;
  left: 2px;
  bottom: 2px;
  background: var(--qa-text-muted);
  border-radius: 50%;
  transition: all var(--qa-transition);
}

.toggle__input:checked + .toggle__slider {
  background: var(--qa-accent);
  border-color: var(--qa-accent);
}

.toggle__input:checked + .toggle__slider::before {
  transform: translateX(18px);
  background: var(--qa-bg);
}

/* Data actions */
.settings__data-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

/* About */
.settings__about {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.settings__about-name {
  font-size: 16px;
  font-weight: 600;
  color: var(--qa-text);
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', 'Fira Code', monospace;
}

.settings__about-desc {
  font-size: 13px;
}

.settings__about-version {
  font-size: 12px;
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', 'Fira Code', monospace;
}

.settings__about-link {
  font-size: 13px;
  color: var(--qa-accent);
  text-decoration: none;
  margin-top: 4px;
}

.settings__about-link:hover {
  color: var(--qa-accent-hover);
  text-decoration: underline;
}

/* Footer */
.settings__footer {
  flex-shrink: 0;
  padding-top: 8px;
  padding-bottom: 8px;
}

/* Fade transition */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 200ms ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
