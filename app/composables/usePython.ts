import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'

export function usePython() {
  const pythonPath = ref<string>('')
  const isDetecting = ref(false)
  const error = ref<string | null>(null)

  async function detect(): Promise<string> {
    isDetecting.value = true
    error.value = null
    try {
      const path = await invoke<string>('detect_python')
      pythonPath.value = path
      return path
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      error.value = message
      console.error('[usePython] Failed to detect Python:', message)
      throw err
    } finally {
      isDetecting.value = false
    }
  }

  return {
    pythonPath,
    isDetecting,
    error,
    detect,
  }
}
