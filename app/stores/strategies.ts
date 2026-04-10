import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { ref, computed } from 'vue'
import type { Strategy } from '~/types'

export const useStrategiesStore = defineStore('strategies', () => {
  // ── State ──

  const strategies = ref<Strategy[]>([])
  const activeStrategyId = ref<string | null>(null)
  const editorContent = ref<string>('')
  const isLoading = ref(false)

  // ── Getters ──

  const activeStrategy = computed(() => {
    if (!activeStrategyId.value) return null
    return strategies.value.find((s) => s.id === activeStrategyId.value) ?? null
  })

  // ── Actions ──

  async function load() {
    isLoading.value = true
    try {
      strategies.value = await invoke<Strategy[]>('list_strategies')
    } catch (err) {
      console.error('[strategies store] Failed to load strategies:', err)
    } finally {
      isLoading.value = false
    }
  }

  async function create(name: string, description: string): Promise<Strategy> {
    try {
      const strategy = await invoke<Strategy>('create_strategy', { name, description })
      await load()
      return strategy
    } catch (err) {
      console.error('[strategies store] Failed to create strategy:', err)
      throw err
    }
  }

  async function save(id: string, code: string, params?: string | null) {
    try {
      await invoke('save_strategy', {
        id,
        code,
        paramsJson: params ?? null,
      })
      // Refresh the strategy list to pick up updated_at changes
      await load()
    } catch (err) {
      console.error('[strategies store] Failed to save strategy:', err)
      throw err
    }
  }

  async function remove(id: string) {
    try {
      await invoke('delete_strategy', { id })
      if (activeStrategyId.value === id) {
        activeStrategyId.value = null
        editorContent.value = ''
      }
      await load()
    } catch (err) {
      console.error('[strategies store] Failed to delete strategy:', err)
      throw err
    }
  }

  async function select(id: string) {
    activeStrategyId.value = id
    try {
      editorContent.value = await readFile(id)
    } catch (err) {
      console.error('[strategies store] Failed to read strategy file:', err)
      editorContent.value = ''
    }
  }

  async function readFile(id: string): Promise<string> {
    try {
      return await invoke<string>('read_strategy_file', { id })
    } catch (err) {
      console.error('[strategies store] Failed to read strategy file:', err)
      throw err
    }
  }

  return {
    // State
    strategies,
    activeStrategyId,
    editorContent,
    isLoading,
    // Getters
    activeStrategy,
    // Actions
    load,
    create,
    save,
    delete: remove,
    select,
    readFile,
  }
})
