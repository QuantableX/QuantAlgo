import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { onMounted, onBeforeUnmount, ref } from 'vue'

export function useTauriEvent<T>(eventName: string, handler: (payload: T) => void) {
  const unlistenFn = ref<UnlistenFn | null>(null)

  onMounted(async () => {
    try {
      unlistenFn.value = await listen<T>(eventName, (event) => {
        handler(event.payload)
      })
    } catch (err) {
      console.error(`[useTauriEvent] Failed to listen to "${eventName}":`, err)
    }
  })

  onBeforeUnmount(() => {
    if (unlistenFn.value) {
      unlistenFn.value()
      unlistenFn.value = null
    }
  })

  async function unlisten() {
    if (unlistenFn.value) {
      unlistenFn.value()
      unlistenFn.value = null
    }
  }

  return { unlisten }
}
