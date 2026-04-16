<script setup lang="ts">
import { useJournalStore } from '~/stores/journal'
import { formatCurrency, formatPct, formatDuration, formatDateTime } from '~/utils/format'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { BotTradeEvent, Trade } from '~/types'

const journalStore = useJournalStore()

// Selected trade for detail panel
const selectedTrade = ref<Trade | null>(null)
const editingNotes = ref('')
const isSavingNotes = ref(false)
let unlistenTrade: UnlistenFn | null = null

function handleSelect(trade: Trade) {
  if (selectedTrade.value?.id === trade.id) {
    selectedTrade.value = null
    return
  }
  selectedTrade.value = trade
  editingNotes.value = trade.notes ?? ''
}

async function handleUpdateNotes(id: string, notes: string) {
  isSavingNotes.value = true
  try {
    await journalStore.updateNotes(id, notes)
    if (selectedTrade.value?.id === id) {
      selectedTrade.value = { ...selectedTrade.value, notes }
    }
  } catch (err) {
    console.error('Failed to save notes:', err)
  } finally {
    isSavingNotes.value = false
  }
}

async function saveNotes() {
  if (!selectedTrade.value) return
  await handleUpdateNotes(selectedTrade.value.id, editingNotes.value)
}

// Watch filters and reload
watch(
  () => journalStore.filters,
  () => {
    journalStore.loadTrades()
    journalStore.loadStats()
  },
  { deep: true },
)

onMounted(async () => {
  await Promise.all([journalStore.loadTrades(), journalStore.loadStats()])
  unlistenTrade = await listen<BotTradeEvent>('bot:trade', async () => {
    await journalStore.refresh()
  })
})

onUnmounted(() => {
  unlistenTrade?.()
})
</script>

<template>
  <div class="journal">
    <!-- Loading State -->
    <div v-if="journalStore.isLoading && !journalStore.trades.length" class="journal__loading">
      <p class="text-muted">Loading trades...</p>
    </div>

    <template v-else>
      <!-- Stats Header -->
      <div class="journal__stats">
        <StatsHeader v-if="journalStore.stats" :stats="journalStore.stats" />
      </div>

      <!-- Calendar Heatmap -->
      <div class="journal__calendar">
        <CalendarHeatmap :trades="journalStore.trades" />
      </div>

      <!-- Trade Table -->
      <div class="journal__table">
        <TradeTable
          :trades="journalStore.trades"
          @select="handleSelect"
          @update-notes="handleUpdateNotes"
        />
      </div>

      <!-- Empty State -->
      <div v-if="!journalStore.trades.length && !journalStore.isLoading" class="journal__empty">
        <p class="empty-state text-muted">
          No trades recorded yet. Start trading or run a backtest to see your journal.
        </p>
      </div>

      <!-- Expanded Trade Detail -->
      <Transition name="slide">
        <div v-if="selectedTrade" class="journal__detail card">
          <div class="detail__header">
            <h3 class="detail__title">Trade Detail</h3>
            <button class="btn btn-sm" @click="selectedTrade = null">Close</button>
          </div>

          <div class="detail__grid">
            <div class="detail__field">
              <span class="detail__label">Pair</span>
              <span class="detail__value">{{ selectedTrade.pair }}</span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Side</span>
              <span
                class="detail__value"
                :class="selectedTrade.side === 'long' ? 'text-accent' : 'text-error'"
              >
                {{ selectedTrade.side.toUpperCase() }}
              </span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Entry Price</span>
              <span class="detail__value mono">{{ formatCurrency(selectedTrade.entry_price) }}</span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Exit Price</span>
              <span class="detail__value mono">
                {{ selectedTrade.exit_price != null ? formatCurrency(selectedTrade.exit_price) : '--' }}
              </span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Quantity</span>
              <span class="detail__value mono">{{ selectedTrade.quantity }}</span>
            </div>
            <div class="detail__field">
              <span class="detail__label">PnL</span>
              <span
                class="detail__value mono"
                :class="(selectedTrade.pnl ?? 0) >= 0 ? 'text-success' : 'text-error'"
              >
                {{ selectedTrade.pnl != null ? formatCurrency(selectedTrade.pnl) : '--' }}
              </span>
            </div>
            <div class="detail__field">
              <span class="detail__label">PnL %</span>
              <span
                class="detail__value mono"
                :class="(selectedTrade.pnl_pct ?? 0) >= 0 ? 'text-success' : 'text-error'"
              >
                {{ selectedTrade.pnl_pct != null ? formatPct(selectedTrade.pnl_pct) : '--' }}
              </span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Fee</span>
              <span class="detail__value mono">{{ formatCurrency(selectedTrade.fee) }}</span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Entry Time</span>
              <span class="detail__value">{{ formatDateTime(selectedTrade.entry_time) }}</span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Exit Time</span>
              <span class="detail__value">
                {{ selectedTrade.exit_time ? formatDateTime(selectedTrade.exit_time) : '--' }}
              </span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Duration</span>
              <span class="detail__value">
                {{
                  selectedTrade.entry_time && selectedTrade.exit_time
                    ? formatDuration(
                        (new Date(selectedTrade.exit_time).getTime() -
                          new Date(selectedTrade.entry_time).getTime()) /
                          1000,
                      )
                    : '--'
                }}
              </span>
            </div>
            <div class="detail__field">
              <span class="detail__label">Exchange</span>
              <span class="detail__value">{{ selectedTrade.exchange }}</span>
            </div>
          </div>

          <div class="detail__notes">
            <label class="label" for="trade-notes">Notes</label>
            <textarea
              id="trade-notes"
              v-model="editingNotes"
              class="input detail__textarea"
              placeholder="Add notes about this trade..."
              rows="4"
            />
            <button
              class="btn btn-primary detail__save-btn"
              :disabled="isSavingNotes"
              @click="saveNotes"
            >
              {{ isSavingNotes ? 'Saving...' : 'Save Notes' }}
            </button>
          </div>
        </div>
      </Transition>
    </template>
  </div>
</template>

<style scoped>
.journal {
  height: 100%;
  overflow-y: auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.journal__loading {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  font-size: 14px;
}

.journal__stats {
  flex-shrink: 0;
}

.journal__calendar {
  flex-shrink: 0;
}

.journal__table {
  flex: 1;
  min-height: 0;
}

.journal__empty {
  text-align: center;
  padding: 40px 0;
}

.empty-state {
  font-size: 13px;
}

/* Detail Panel */
.journal__detail {
  flex-shrink: 0;
}

.detail__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}

.detail__title {
  font-size: 14px;
  font-weight: 600;
  color: var(--qa-text);
}

.detail__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
  margin-bottom: 16px;
}

.detail__field {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.detail__label {
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--qa-text-muted);
}

.detail__value {
  font-size: 13px;
  color: var(--qa-text);
  font-weight: 500;
}

.detail__notes {
  border-top: 1px solid var(--qa-border-subtle);
  padding-top: 16px;
}

.detail__textarea {
  resize: vertical;
  min-height: 80px;
  margin-bottom: 8px;
  font-family: inherit;
  line-height: 1.5;
}

.detail__save-btn {
  margin-top: 4px;
}

/* Slide transition */
.slide-enter-active,
.slide-leave-active {
  transition: all 200ms ease;
}

.slide-enter-from,
.slide-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
