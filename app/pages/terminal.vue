<script setup lang="ts">
const botTerminalRef = ref<{ clear: () => void; getContent?: () => string } | null>(null)
const autoScroll = ref(true)

function handleClear() {
  if (botTerminalRef.value) {
    botTerminalRef.value.clear()
  }
}

function handleExport() {
  if (!botTerminalRef.value) return
  const content = botTerminalRef.value.getContent?.() ?? ''
  if (!content) return

  const blob = new Blob([content], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `quantalgo-log-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.txt`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

function toggleAutoScroll() {
  autoScroll.value = !autoScroll.value
}
</script>

<template>
  <div class="terminal-page">
    <!-- Toolbar -->
    <div class="terminal-toolbar">
      <div class="terminal-toolbar__left">
        <h2 class="terminal-toolbar__title">Bot Terminal</h2>
      </div>
      <div class="terminal-toolbar__right">
        <button
          class="btn btn-sm"
          :class="{ 'btn--active': autoScroll }"
          @click="toggleAutoScroll"
        >
          Auto-scroll {{ autoScroll ? 'On' : 'Off' }}
        </button>
        <button class="btn btn-sm" @click="handleClear">Clear</button>
        <button class="btn btn-sm" @click="handleExport">Export Log</button>
      </div>
    </div>

    <!-- Terminal -->
    <div class="terminal-container">
      <BotTerminal
        ref="botTerminalRef"
        :auto-scroll="autoScroll"
      />
    </div>
  </div>
</template>

<style scoped>
.terminal-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.terminal-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 44px;
  flex-shrink: 0;
  padding: 0 16px;
  background: var(--qa-bg-card);
  border-bottom: 1px solid var(--qa-border);
}

.terminal-toolbar__left {
  display: flex;
  align-items: center;
}

.terminal-toolbar__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--qa-text);
}

.terminal-toolbar__right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn--active {
  background: var(--qa-accent);
  color: var(--qa-bg);
  border-color: var(--qa-accent);
}

.btn--active:hover {
  background: var(--qa-accent-hover);
  border-color: var(--qa-accent-hover);
}

.terminal-container {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
</style>
