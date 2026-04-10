import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import type { Trade, TradeStats, TradeFilters } from '~/types'

export const useJournalStore = defineStore('journal', () => {
  // ── State ──

  const trades = ref<Trade[]>([])
  const stats = ref<TradeStats | null>(null)
  const filters = ref<TradeFilters>({
    limit: 50,
    offset: 0,
  })
  const isLoading = ref(false)
  const totalCount = ref(0)

  // ── Actions ──

  async function loadTrades() {
    isLoading.value = true
    try {
      trades.value = await invoke<Trade[]>('list_trades', { filters: filters.value })
    } catch (err) {
      console.error('[journal store] Failed to load trades:', err)
    } finally {
      isLoading.value = false
    }
  }

  async function loadStats() {
    try {
      stats.value = await invoke<TradeStats>('get_trade_stats', { filters: filters.value })
    } catch (err) {
      console.error('[journal store] Failed to load trade stats:', err)
    }
  }

  async function updateFilters(partial: Partial<TradeFilters>) {
    // Reset offset when filters change (unless offset itself is being set)
    if (partial.offset === undefined) {
      filters.value = { ...filters.value, ...partial, offset: 0 }
    } else {
      filters.value = { ...filters.value, ...partial }
    }
    await Promise.all([loadTrades(), loadStats()])
  }

  async function updateNotes(id: string, notes: string) {
    try {
      await invoke('update_trade_notes', { id, notes })
      // Update the local trade record
      const trade = trades.value.find((t) => t.id === id)
      if (trade) {
        trade.notes = notes
      }
    } catch (err) {
      console.error('[journal store] Failed to update trade notes:', err)
      throw err
    }
  }

  async function refresh() {
    await Promise.all([loadTrades(), loadStats()])
  }

  function resetFilters() {
    filters.value = { limit: 50, offset: 0 }
  }

  return {
    // State
    trades,
    stats,
    filters,
    isLoading,
    totalCount,
    // Actions
    loadTrades,
    loadStats,
    updateFilters,
    updateNotes,
    refresh,
    resetFilters,
  }
})
