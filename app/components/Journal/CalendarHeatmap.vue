<script setup lang="ts">
import type { Trade } from '~/types'
import { formatCurrency } from '~/utils/format'

const props = defineProps<{
  trades: Trade[]
}>()

const WEEKS = 13
const CELL_SIZE = 14
const CELL_GAP = 2
const DAY_LABELS = ['M', '', 'W', '', 'F', '', '']

interface DayCell {
  date: string
  pnl: number
  level: number
  hasData: boolean
}

const dailyPnl = computed<Record<string, number>>(() => {
  const map: Record<string, number> = {}
  for (const trade of props.trades) {
    if (trade.pnl === null) continue
    const dateStr = trade.exit_time
      ? trade.exit_time.slice(0, 10)
      : trade.entry_time.slice(0, 10)
    map[dateStr] = (map[dateStr] ?? 0) + trade.pnl
  }
  return map
})

const grid = computed<{ cells: DayCell[][]; monthLabels: { label: string; col: number }[] }>(() => {
  const today = new Date()
  const todayDay = today.getDay()
  const mondayOffset = todayDay === 0 ? 6 : todayDay - 1
  const endMonday = new Date(today)
  endMonday.setDate(today.getDate() - mondayOffset)

  const startDate = new Date(endMonday)
  startDate.setDate(startDate.getDate() - (WEEKS - 1) * 7)

  const pnlValues = Object.values(dailyPnl.value)
  const maxProfit = Math.max(0, ...pnlValues.filter((v) => v > 0))
  const maxLoss = Math.min(0, ...pnlValues.filter((v) => v < 0))

  const columns: DayCell[][] = []
  const monthLabels: { label: string; col: number }[] = []
  let lastMonth = -1

  for (let w = 0; w < WEEKS; w++) {
    const week: DayCell[] = []
    for (let d = 0; d < 7; d++) {
      const cellDate = new Date(startDate)
      cellDate.setDate(startDate.getDate() + w * 7 + d)
      const dateStr = cellDate.toISOString().slice(0, 10)
      const pnl = dailyPnl.value[dateStr] ?? 0
      const hasData = dateStr in dailyPnl.value

      let level = 0
      if (hasData) {
        if (pnl > 0 && maxProfit > 0) {
          const ratio = pnl / maxProfit
          level = ratio < 0.25 ? 1 : ratio < 0.5 ? 2 : ratio < 0.75 ? 3 : 4
        } else if (pnl < 0 && maxLoss < 0) {
          const ratio = pnl / maxLoss
          level = ratio < 0.25 ? -1 : ratio < 0.5 ? -2 : ratio < 0.75 ? -3 : -4
        }
      }

      const cellMonth = cellDate.getMonth()
      if (d === 0 && cellMonth !== lastMonth) {
        const monthName = cellDate.toLocaleString('en-US', { month: 'short' })
        monthLabels.push({ label: monthName, col: w })
        lastMonth = cellMonth
      }

      week.push({ date: dateStr, pnl, level, hasData })
    }
    columns.push(week)
  }

  return { cells: columns, monthLabels }
})

function cellColor(level: number, hasData: boolean): string {
  if (!hasData) return 'var(--qa-bg-card)'
  switch (level) {
    case 4: return '#1a7f37'
    case 3: return '#26a641'
    case 2: return '#39d353'
    case 1: return '#57e06c'
    case -1: return '#ff8a8a'
    case -2: return '#ff6b6b'
    case -3: return '#ff4757'
    case -4: return '#cc2233'
    default: return 'var(--qa-bg-card)'
  }
}

const hoveredCell = ref<DayCell | null>(null)
const tooltipPos = ref({ x: 0, y: 0 })

function onCellHover(cell: DayCell, event: MouseEvent) {
  hoveredCell.value = cell
  tooltipPos.value = { x: event.clientX, y: event.clientY }
}

function onCellLeave() {
  hoveredCell.value = null
}

function formatDateLabel(dateStr: string): string {
  const d = new Date(dateStr)
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
}

const totalWidth = computed(() => WEEKS * (CELL_SIZE + CELL_GAP) + 24)
</script>

<template>
  <div class="heatmap-container">
    <div class="heatmap-scroll">
      <div class="heatmap-grid" :style="{ minWidth: `${totalWidth}px` }">
        <!-- Month labels -->
        <div class="month-labels" :style="{ paddingLeft: '24px' }">
          <span
            v-for="ml in grid.monthLabels"
            :key="ml.col"
            class="month-label"
            :style="{ left: `${24 + ml.col * (CELL_SIZE + CELL_GAP)}px` }"
          >
            {{ ml.label }}
          </span>
        </div>

        <div class="grid-body">
          <!-- Day-of-week labels -->
          <div class="day-labels">
            <span
              v-for="(label, i) in DAY_LABELS"
              :key="i"
              class="day-label"
              :style="{ height: `${CELL_SIZE}px`, marginBottom: `${CELL_GAP}px` }"
            >
              {{ label }}
            </span>
          </div>

          <!-- Cell columns -->
          <div class="cells-area">
            <div
              v-for="(week, wi) in grid.cells"
              :key="wi"
              class="week-column"
            >
              <span
                v-for="(cell, di) in week"
                :key="di"
                class="day-cell"
                :style="{
                  width: `${CELL_SIZE}px`,
                  height: `${CELL_SIZE}px`,
                  backgroundColor: cellColor(cell.level, cell.hasData),
                  marginBottom: `${CELL_GAP}px`,
                }"
                @mouseenter="onCellHover(cell, $event)"
                @mouseleave="onCellLeave"
              />
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Tooltip -->
    <Teleport to="body">
      <div
        v-if="hoveredCell"
        class="heatmap-tooltip"
        :style="{
          left: `${tooltipPos.x + 12}px`,
          top: `${tooltipPos.y - 8}px`,
        }"
      >
        <span class="tooltip-date">{{ formatDateLabel(hoveredCell.date) }}</span>
        <span
          v-if="hoveredCell.hasData"
          class="tooltip-pnl"
          :class="hoveredCell.pnl >= 0 ? 'text-success' : 'text-error'"
        >
          {{ formatCurrency(hoveredCell.pnl) }}
        </span>
        <span v-else class="tooltip-pnl text-muted">No trades</span>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.heatmap-container {
  position: relative;
}

.heatmap-scroll {
  overflow-x: auto;
  padding-bottom: 4px;
}

.heatmap-grid {
  display: flex;
  flex-direction: column;
}

.month-labels {
  position: relative;
  height: 18px;
  margin-bottom: 4px;
}

.month-label {
  position: absolute;
  font-size: 11px;
  color: var(--qa-text-muted);
  font-weight: 500;
}

.grid-body {
  display: flex;
}

.day-labels {
  display: flex;
  flex-direction: column;
  width: 20px;
  flex-shrink: 0;
  margin-right: 4px;
}

.day-label {
  display: flex;
  align-items: center;
  font-size: 10px;
  color: var(--qa-text-muted);
  line-height: 1;
}

.cells-area {
  display: flex;
  gap: 2px;
}

.week-column {
  display: flex;
  flex-direction: column;
}

.day-cell {
  border-radius: 2px;
  cursor: pointer;
  transition: opacity var(--qa-transition);
}

.day-cell:hover {
  opacity: 0.8;
  outline: 1px solid var(--qa-text-muted);
}

.heatmap-tooltip {
  position: fixed;
  z-index: 200;
  background: var(--qa-bg-card);
  border: 1px solid var(--qa-border);
  border-radius: var(--qa-radius);
  padding: 6px 10px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  pointer-events: none;
  white-space: nowrap;
}

.tooltip-date {
  font-size: 12px;
  color: var(--qa-text);
  font-weight: 500;
}

.tooltip-pnl {
  font-size: 12px;
  font-weight: 600;
}
</style>
