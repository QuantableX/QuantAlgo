<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useBotStore } from '~/stores/bot'
import { useStrategiesStore } from '~/stores/strategies'
import logoUrl from '~/assets/images/QuantAlgo.png'

const bot = useBotStore()
const strategies = useStrategiesStore()

// ── Sidebar collapse ──

const leftCollapsed = ref(false)
const rightCollapsed = ref(false)

// ── Settings modal ──

const settingsOpen = ref(false)

// ── Window controls (exact QuantCode pattern) ──

const appWindow = getCurrentWindow()
const isMaximized = ref(false)

async function minimizeWindow() {
  await appWindow.minimize()
}

async function toggleMaximize() {
  await appWindow.toggleMaximize()
  isMaximized.value = await appWindow.isMaximized()
}

async function closeWindow() {
  await appWindow.close()
}

function onTitlebarMousedown(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('button, input, select, a, .window-controls, .settings-btn, .titlebar-logo-section')) return
  appWindow.startDragging()
}

function onTitlebarDblclick(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('button, input, select, a, .window-controls, .settings-btn, .titlebar-logo-section')) return
  toggleMaximize()
}

// ── Status computeds ──

const statusColor = computed(() => {
  if (bot.status === 'running') return 'var(--qa-accent)'
  if (bot.status === 'error') return 'var(--qa-error)'
  return 'var(--qa-text-muted)'
})

const statusLabel = computed(() => {
  if (bot.status === 'running') return 'Running'
  if (bot.status === 'error') return 'Error'
  return 'Stopped'
})

const activeStrategyName = computed(() => {
  if (!bot.activeStrategyId) return null
  return strategies.strategies.find(s => s.id === bot.activeStrategyId)?.name ?? null
})

const pnlDisplay = computed(() => {
  const v = bot.pnl.today
  return `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`
})

const pnlPositive = computed(() => bot.pnl.today >= 0)
const openCount = computed(() => bot.openPositions.length)

// ── Keyboard shortcuts ──

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  appWindow.isMaximized().then(v => { isMaximized.value = v })
  appWindow.onResized(async () => {
    isMaximized.value = await appWindow.isMaximized()
  })
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})

function onKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && !e.shiftKey && e.key === 'b') {
    e.preventDefault()
    leftCollapsed.value = !leftCollapsed.value
  }
  if (e.ctrlKey && e.shiftKey && e.key === 'B') {
    e.preventDefault()
    rightCollapsed.value = !rightCollapsed.value
  }
}
</script>

<template>
  <div class="app-shell">
    <!-- Titlebar -->
    <div class="titlebar" @mousedown="onTitlebarMousedown" @dblclick="onTitlebarDblclick">
      <!-- Left: Logo -->
      <div
        class="titlebar-logo-section"
        style="-webkit-app-region: no-drag; cursor: pointer;"
        @click="router.push('/')"
      >
        <img :src="logoUrl" alt="QuantAlgo" class="titlebar-logo" draggable="false" />
      </div>

      <!-- Center: Status info -->
      <div class="titlebar-center">
        <div class="status-left">
          <span
            class="status-dot"
            :class="{ 'status-dot--pulse': bot.status === 'running' }"
            :style="{ background: statusColor }"
          />
          <span class="status-label">Bot:&nbsp;</span>
          <span class="status-value" :style="{ color: statusColor }">{{ statusLabel }}</span>
          <span v-if="activeStrategyName" class="strategy-chip">{{ activeStrategyName }}</span>
        </div>
        <div class="status-right">
          <span class="metric-pill" :class="pnlPositive ? 'metric-pill--pos' : 'metric-pill--neg'">
            PNL: {{ pnlDisplay }}
          </span>
          <span class="metric-pill">OPEN: {{ openCount }}</span>
        </div>
      </div>

      <!-- Right: Settings + Window Controls -->
      <div class="titlebar-actions-section" style="-webkit-app-region: no-drag">
        <button
          class="settings-btn"
          title="Settings"
          @click.stop="settingsOpen = true"
        >&#9881;</button>
        <div class="window-controls">
          <button class="window-btn" title="Minimize" @click="minimizeWindow">
            <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.2" /></svg>
          </button>
          <button class="window-btn" :title="isMaximized ? 'Restore' : 'Maximize'" @click="toggleMaximize">
            <svg v-if="!isMaximized" width="12" height="12" viewBox="0 0 12 12"><rect x="1.5" y="1.5" width="9" height="9" rx="1" stroke="currentColor" stroke-width="1.2" fill="none" /></svg>
            <svg v-else width="12" height="12" viewBox="0 0 12 12"><rect x="2.5" y="3.5" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.2" fill="none" /><path d="M4.5 3.5V2.5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H8.5" stroke="currentColor" stroke-width="1.2" fill="none" /></svg>
          </button>
          <button class="window-btn window-btn-close" title="Close" @click="closeWindow">
            <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.2" /></svg>
          </button>
        </div>
      </div>
    </div>

    <!-- Body -->
    <div class="app-body">
      <div class="sidebar-left" :class="{ collapsed: leftCollapsed }">
        <LayoutSidebar />
      </div>
      <main class="main-content">
        <slot />
      </main>
      <div class="sidebar-right" :class="{ collapsed: rightCollapsed }">
        <LayoutRightSidebar />
      </div>
    </div>

    <UISettingsModal :visible="settingsOpen" @close="settingsOpen = false" />
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}

/* ── Titlebar ── */

.titlebar {
  flex-shrink: 0;
  display: flex;
  align-items: stretch;
  background: var(--qa-bg-sidebar);
  border-bottom: 1px solid var(--qa-border);
  height: 51px;
}

.titlebar-logo-section {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-right: 1px solid var(--qa-border);
  padding: 0 16px;
}

.titlebar-logo {
  height: 22px;
  width: auto;
  object-fit: contain;
}

/* ── Center: status ── */

.titlebar-center {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
}

.status-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-dot--pulse {
  animation: pulse-dot 2s ease-in-out infinite;
}

.status-label {
  font-size: 12px;
  color: var(--qa-text-muted);
}

.status-value {
  font-size: 12px;
  font-weight: 600;
}

.strategy-chip {
  padding: 2px 8px;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  border: 1px solid var(--qa-border);
  border-radius: 9999px;
  color: var(--qa-text-secondary);
}

.status-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.metric-pill {
  padding: 3px 10px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  border: 1px solid var(--qa-border);
  border-radius: 9999px;
  color: var(--qa-text-secondary);
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', Menlo, Consolas, monospace;
}

.metric-pill--pos {
  color: var(--qa-success);
  border-color: color-mix(in srgb, var(--qa-success) 30%, transparent);
}

.metric-pill--neg {
  color: var(--qa-error);
  border-color: color-mix(in srgb, var(--qa-error) 30%, transparent);
}

/* ── Right: Actions ── */

.titlebar-actions-section {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 0 0 8px;
  border-left: 1px solid var(--qa-border);
}

.settings-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 999px;
  border: 1px solid var(--qa-border);
  background: var(--qa-bg-card);
  color: var(--qa-text-muted);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
  margin: 0 auto;
}

.settings-btn:hover {
  color: var(--qa-text);
  border-color: var(--qa-accent);
  background: var(--qa-bg-hover);
}

/* ── Window controls ── */

.window-controls {
  display: flex;
  align-items: stretch;
  height: 100%;
  flex-shrink: 0;
}

.window-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 51px;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--qa-text-muted);
  cursor: pointer;
  transition: background 0.1s, color 0.1s;
}

.window-btn:hover {
  background: color-mix(in srgb, var(--qa-text) 10%, transparent);
  color: var(--qa-text);
}

.window-btn:active {
  background: color-mix(in srgb, var(--qa-text) 16%, transparent);
}

.window-btn-close:hover {
  background: #e81123;
  color: #fff;
}

.window-btn-close:active {
  background: #c50f1f;
  color: #fff;
}

/* ── Body ── */

.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.sidebar-left {
  width: 220px;
  min-width: 220px;
  background: var(--qa-bg-sidebar);
  border-right: 1px solid var(--qa-border);
  overflow-y: auto;
  overflow-x: hidden;
  transition: width 0.15s, min-width 0.15s;
}

.sidebar-left.collapsed {
  width: 0;
  min-width: 0;
  border-right: none;
}

.main-content {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 24px 32px;
  background: var(--qa-bg);
}

.sidebar-right {
  width: 220px;
  min-width: 220px;
  background: var(--qa-bg-sidebar);
  border-left: 1px solid var(--qa-border);
  overflow-y: auto;
  overflow-x: hidden;
  transition: width 0.15s, min-width 0.15s;
}

.sidebar-right.collapsed {
  width: 0;
  min-width: 0;
  border-left: none;
}
</style>
