import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import type { AppSettings } from '~/types'

export const useAppStore = defineStore('app', () => {
  // ── State ──

  const settings = ref<AppSettings>({
    theme: 'dark',
    font_size: 14,
    default_pair: 'BTC/USDT',
    default_timeframe: '1h',
    python_path: '',
    strategy_dir: '',
    backtest_dir: '',
    risk_per_trade: 1,
    max_concurrent_positions: 3,
    slippage_tolerance: 0.1,
    notify_on_trade: true,
    notify_on_error: true,
    notify_on_daily_summary: false,
  })

  const isLoading = ref(false)

  // ── Actions ──

  async function loadSettings() {
    isLoading.value = true
    try {
      const loaded = await invoke<AppSettings>('get_settings')
      settings.value = loaded
      applyTheme(loaded.theme)
      applyFontSize(loaded.font_size)
    } catch (err) {
      console.error('[app store] Failed to load settings:', err)
    } finally {
      isLoading.value = false
    }
  }

  async function saveSettings(partial: Partial<AppSettings>) {
    const merged = { ...settings.value, ...partial }
    try {
      await invoke('update_settings', { settings: merged })
      settings.value = merged

      if (partial.theme !== undefined) {
        applyTheme(partial.theme)
      }
      if (partial.font_size !== undefined) {
        applyFontSize(partial.font_size)
      }
    } catch (err) {
      console.error('[app store] Failed to save settings:', err)
      throw err
    }
  }

  function applyTheme(theme: 'dark' | 'light') {
    if (import.meta.client) {
      document.documentElement.setAttribute('data-theme', theme)
      document.documentElement.classList.toggle('dark', theme === 'dark')
      localStorage.setItem('quantalgo-theme', theme)
    }
    settings.value.theme = theme
  }

  function applyFontSize(size: number) {
    if (import.meta.client) {
      document.documentElement.style.setProperty('--app-font-size', `${size}px`)
    }
    settings.value.font_size = size
  }

  async function detectPython(): Promise<string> {
    try {
      const path = await invoke<string>('detect_python')
      settings.value.python_path = path
      return path
    } catch (err) {
      console.error('[app store] Failed to detect Python:', err)
      throw err
    }
  }

  return {
    settings,
    isLoading,
    loadSettings,
    saveSettings,
    applyTheme,
    applyFontSize,
    detectPython,
  }
})
