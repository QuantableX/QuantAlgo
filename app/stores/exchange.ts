import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { ref, computed } from 'vue'
import type { Exchange, ExchangeConfig, ConnectionResult, Balance } from '~/types'

export const useExchangeStore = defineStore('exchange', () => {
  // ── State ──

  const exchanges = ref<Exchange[]>([])
  const activeExchangeId = ref<string | null>(null)
  const balances = ref<Balance[]>([])
  const pairs = ref<string[]>([])
  const isLoading = ref(false)

  // ── Getters ──

  const activeExchange = computed(() => {
    if (!activeExchangeId.value) return null
    return exchanges.value.find((e) => e.id === activeExchangeId.value) ?? null
  })

  // ── Actions ──

  async function load() {
    isLoading.value = true
    try {
      exchanges.value = await invoke<Exchange[]>('list_exchanges')
    } catch (err) {
      console.error('[exchange store] Failed to load exchanges:', err)
    } finally {
      isLoading.value = false
    }
  }

  async function add(config: ExchangeConfig): Promise<Exchange> {
    try {
      const exchange = await invoke<Exchange>('add_exchange', { config })
      await load()
      return exchange
    } catch (err) {
      console.error('[exchange store] Failed to add exchange:', err)
      throw err
    }
  }

  async function update(id: string, config: Partial<ExchangeConfig>) {
    try {
      await invoke('update_exchange', { id, config })
      await load()
    } catch (err) {
      console.error('[exchange store] Failed to update exchange:', err)
      throw err
    }
  }

  async function remove(id: string) {
    try {
      await invoke('delete_exchange', { id })
      if (activeExchangeId.value === id) {
        activeExchangeId.value = null
        balances.value = []
        pairs.value = []
      }
      await load()
    } catch (err) {
      console.error('[exchange store] Failed to delete exchange:', err)
      throw err
    }
  }

  async function testConnection(id: string): Promise<ConnectionResult> {
    try {
      return await invoke<ConnectionResult>('test_exchange_connection', { id })
    } catch (err) {
      console.error('[exchange store] Failed to test connection:', err)
      throw err
    }
  }

  async function refreshBalances(exchangeId: string) {
    try {
      balances.value = await invoke<Balance[]>('get_balances', { exchangeId })
    } catch (err) {
      console.error('[exchange store] Failed to refresh balances:', err)
      throw err
    }
  }

  async function loadPairs(exchangeId: string): Promise<string[]> {
    try {
      const result = await invoke<string[]>('get_exchange_pairs', { exchangeId })
      pairs.value = result
      return result
    } catch (err) {
      console.error('[exchange store] Failed to load pairs:', err)
      throw err
    }
  }

  function setActive(id: string | null) {
    activeExchangeId.value = id
    if (!id) {
      balances.value = []
      pairs.value = []
    }
  }

  return {
    // State
    exchanges,
    activeExchangeId,
    balances,
    pairs,
    isLoading,
    // Getters
    activeExchange,
    // Actions
    load,
    add,
    update,
    delete: remove,
    testConnection,
    refreshBalances,
    loadPairs,
    setActive,
  }
})
