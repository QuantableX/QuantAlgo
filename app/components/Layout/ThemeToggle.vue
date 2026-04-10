<script setup lang="ts">
import { useAppStore } from '~/stores/app'

const app = useAppStore()

const isDark = computed(() => app.settings.theme === 'dark')

function toggle() {
  app.applyTheme(isDark.value ? 'light' : 'dark')
}
</script>

<template>
  <div class="theme-toggle" role="radiogroup" aria-label="Theme toggle">
    <button
      class="toggle-segment"
      :class="{ active: !isDark }"
      aria-label="Light theme"
      @click="toggle"
    >
      &#9728;
    </button>
    <button
      class="toggle-segment"
      :class="{ active: isDark }"
      aria-label="Dark theme"
      @click="toggle"
    >
      &#9790;
    </button>
  </div>
</template>

<style scoped>
.theme-toggle {
  display: inline-flex;
  align-items: center;
  border: 1px solid var(--qa-border);
  border-radius: 9999px;
  height: 26px;
  padding: 2px;
  gap: 0;
  overflow: hidden;
  flex-shrink: 0;
}

.toggle-segment {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 3px 8px;
  font-size: 12px;
  line-height: 1;
  background: transparent;
  color: var(--qa-text-muted);
  border: none;
  border-radius: 9999px;
  cursor: pointer;
  transition: background var(--qa-transition), color var(--qa-transition);
  height: 100%;
}

.toggle-segment.active {
  background: var(--qa-bg-hover);
  color: var(--qa-text);
}

.toggle-segment:not(.active):hover {
  color: var(--qa-text-secondary);
}
</style>
