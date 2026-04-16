<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { BotLogEvent, LogEntry } from '~/types'

const props = withDefaults(
  defineProps<{
    autoScroll?: boolean
  }>(),
  {
    autoScroll: true,
  },
)

const containerRef = ref<HTMLDivElement | null>(null)

let term: import('@xterm/xterm').Terminal | null = null
let fitAddon: import('@xterm/addon-fit').FitAddon | null = null
let unlisten: UnlistenFn | null = null
let resizeObserver: ResizeObserver | null = null

function formatTimestamp(iso: string): string {
  const d = new Date(iso)
  const h = String(d.getHours()).padStart(2, '0')
  const m = String(d.getMinutes()).padStart(2, '0')
  const s = String(d.getSeconds()).padStart(2, '0')
  return `${h}:${m}:${s}`
}

function colorForLevel(level: string): string {
  switch (level.toLowerCase()) {
    case 'trade': return '\x1b[37m'  // white (monochrome)
    case 'warn':  return '\x1b[33m'  // yellow
    case 'error': return '\x1b[31m'  // red
    default:      return '\x1b[0m'   // default
  }
}

function writeLine(payload: BotLogEvent) {
  if (!term) return
  const ts = formatTimestamp(payload.timestamp)
  const color = colorForLevel(payload.level)
  const reset = '\x1b[0m'
  const dimTs = `\x1b[90m[${ts}]${reset} `
  term.writeln(`${dimTs}${color}${payload.message}${reset}`)

  if (props.autoScroll) {
    term.scrollToBottom()
  }
}

function clear() {
  term?.clear()
}

function getContent(): string {
  if (!term) return ''
  const buffer = term.buffer.active
  const lines: string[] = []
  for (let i = 0; i < buffer.length; i++) {
    const line = buffer.getLine(i)
    if (line) {
      lines.push(line.translateToString(true))
    }
  }
  // Remove trailing empty lines
  while (lines.length > 0) {
    const lastLine = lines[lines.length - 1]
    if (!lastLine || lastLine.trim() !== '') {
      break
    }
    lines.pop()
  }
  return lines.join('\n')
}

defineExpose({ clear, getContent })

onMounted(async () => {
  if (!containerRef.value) return

  const { Terminal } = await import('@xterm/xterm')
  const { FitAddon } = await import('@xterm/addon-fit')
  const { Unicode11Addon } = await import('@xterm/addon-unicode11')

  // Import xterm CSS
  await import('@xterm/xterm/css/xterm.css')

  term = new Terminal({
    theme: {
      background: '#18181e',
      foreground: '#d4d4d8',
      cursor: '#d4d4d8',
      cursorAccent: '#18181e',
      selectionBackground: '#313139',
      black: '#18181e',
      red: '#ff4757',
      green: '#a0a0a8',
      yellow: '#ffa502',
      blue: '#a0a0a8',
      magenta: '#a0a0a8',
      cyan: '#a0a0a8',
      white: '#d4d4d8',
    },
    fontFamily: "ui-monospace, 'Cascadia Code', 'JetBrains Mono', Menlo, Consolas, monospace",
    fontSize: 13,
    lineHeight: 1.4,
    cursorBlink: false,
    disableStdin: true,
    convertEol: true,
  })

  // Load addons
  fitAddon = new FitAddon()
  term.loadAddon(fitAddon)

  const unicode11 = new Unicode11Addon()
  term.loadAddon(unicode11)
  term.unicode.activeVersion = '11'

  // Open terminal in container
  term.open(containerRef.value)

  // Try loading WebGL addon for performance
  try {
    const { WebglAddon } = await import('@xterm/addon-webgl')
    const webgl = new WebglAddon()
    webgl.onContextLoss(() => {
      webgl.dispose()
    })
    term.loadAddon(webgl)
  } catch {
    // WebGL not available, fall back to canvas renderer
  }

  // Fit to container
  fitAddon.fit()

  // Resize observer
  resizeObserver = new ResizeObserver(() => {
    fitAddon?.fit()
  })
  if (containerRef.value) {
    resizeObserver.observe(containerRef.value)
  }

  // Load historical logs so we don't miss startup/preflight output
  try {
    const historicalLogs = await invoke<LogEntry[]>('get_bot_logs', { limit: 500, offset: 0 })
    if (historicalLogs.length > 0) {
      term.writeln('\x1b[90m-- Historical logs --\x1b[0m')
      for (const log of historicalLogs) {
        writeLine({
          timestamp: log.timestamp,
          level: log.level,
          message: log.message,
        })
      }
      term.writeln('\x1b[90m-- Current session --\x1b[0m')
    }
  } catch {
    // Historical logs unavailable, continue with new events only
  }

  // Listen for bot log events from Tauri
  unlisten = await listen<BotLogEvent>('bot:log', (event) => {
    writeLine(event.payload)
  })
})

watch(
  () => props.autoScroll,
  (enabled) => {
    if (enabled && term) {
      term.scrollToBottom()
    }
  },
)

onBeforeUnmount(() => {
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
  if (unlisten) {
    unlisten()
    unlisten = null
  }
  if (term) {
    term.dispose()
    term = null
  }
  fitAddon = null
})
</script>

<template>
  <div ref="containerRef" class="bot-terminal" />
</template>

<style scoped>
.bot-terminal {
  width: 100%;
  height: 100%;
  min-height: 200px;
}

.bot-terminal :deep(.xterm) {
  padding: 8px;
  height: 100%;
}

.bot-terminal :deep(.xterm-viewport) {
  overflow-y: auto !important;
}
</style>
