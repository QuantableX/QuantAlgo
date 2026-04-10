import { useLocalStorage } from '@vueuse/core'
import { computed, watch } from 'vue'

export type Theme = 'dark' | 'light'

export function useTheme() {
  const theme = useLocalStorage<Theme>('quantalgo-theme', 'dark')
  const isDark = computed(() => theme.value === 'dark')

  function applyTheme(value: Theme) {
    if (import.meta.client) {
      document.documentElement.setAttribute('data-theme', value)
      document.documentElement.classList.toggle('dark', value === 'dark')
    }
  }

  function setTheme(value: Theme) {
    theme.value = value
    applyTheme(value)
  }

  function toggleTheme() {
    setTheme(isDark.value ? 'light' : 'dark')
  }

  // Apply the stored theme whenever the ref changes
  watch(theme, (value) => {
    applyTheme(value)
  }, { immediate: true })

  return {
    theme,
    isDark,
    setTheme,
    toggleTheme,
  }
}
