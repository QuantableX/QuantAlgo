<script setup lang="ts">
import { reactive } from 'vue'
import { useStrategiesStore } from '~/stores/strategies'
import type { Strategy, BacktestConfig } from '~/types'

const strategiesStore = useStrategiesStore()

const props = withDefaults(
  defineProps<{
    strategies?: Strategy[]
    defaultStrategyId?: string
    preSelectedStrategyId?: string | null
  }>(),
  {
    strategies: undefined,
    defaultStrategyId: '',
    preSelectedStrategyId: null,
  },
)

const emit = defineEmits<{
  run: [config: BacktestConfig]
}>()

// Resolve strategies: use prop if provided, fall back to store
const resolvedStrategies = computed(() =>
  props.strategies ?? strategiesStore.strategies,
)

// Resolve initial strategy ID from either prop
const initialStrategyId = computed(() =>
  props.preSelectedStrategyId ?? props.defaultStrategyId ?? '',
)

function toDateString(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

const now = new Date()
const thirtyDaysAgo = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000)

const form = reactive<BacktestConfig>({
  strategy_id: initialStrategyId.value,
  exchange: 'binance',
  pair: 'BTC/USDT',
  timeframe: '1h',
  start_date: toDateString(thirtyDaysAgo),
  end_date: toDateString(now),
  initial_capital: 10000,
  commission: 0.1,
})

// Sync strategy ID when props change
watch(
  initialStrategyId,
  (val: string) => {
    if (val) form.strategy_id = val
  },
)

const timeframes = [
  { value: '1m', label: '1 Minute' },
  { value: '5m', label: '5 Minutes' },
  { value: '15m', label: '15 Minutes' },
  { value: '1h', label: '1 Hour' },
  { value: '4h', label: '4 Hours' },
  { value: '1d', label: '1 Day' },
]

function handleSubmit() {
  emit('run', { ...form })
}
</script>

<template>
  <form class="card config-form" @submit.prevent="handleSubmit">
    <div class="form-grid">
      <!-- Row 1 -->
      <div class="field">
        <label class="label" for="bt-strategy">Strategy</label>
        <select
          id="bt-strategy"
          v-model="form.strategy_id"
          class="input"
        >
          <option value="" disabled>Select a strategy</option>
          <option
            v-for="strat in resolvedStrategies"
            :key="strat.id"
            :value="strat.id"
          >
            {{ strat.name }}
          </option>
        </select>
      </div>
      <div class="field">
        <label class="label" for="bt-exchange">Exchange</label>
        <input
          id="bt-exchange"
          v-model="form.exchange"
          class="input"
          type="text"
          placeholder="binance"
        />
      </div>

      <!-- Row 2 -->
      <div class="field">
        <label class="label" for="bt-pair">Trading Pair</label>
        <input
          id="bt-pair"
          v-model="form.pair"
          class="input"
          type="text"
          placeholder="BTC/USDT"
        />
      </div>
      <div class="field">
        <label class="label" for="bt-timeframe">Timeframe</label>
        <select
          id="bt-timeframe"
          v-model="form.timeframe"
          class="input"
        >
          <option
            v-for="tf in timeframes"
            :key="tf.value"
            :value="tf.value"
          >
            {{ tf.label }}
          </option>
        </select>
      </div>

      <!-- Row 3 -->
      <div class="field">
        <label class="label" for="bt-start">Start Date</label>
        <input
          id="bt-start"
          v-model="form.start_date"
          class="input"
          type="date"
        />
      </div>
      <div class="field">
        <label class="label" for="bt-end">End Date</label>
        <input
          id="bt-end"
          v-model="form.end_date"
          class="input"
          type="date"
        />
      </div>

      <!-- Row 4 -->
      <div class="field">
        <label class="label" for="bt-capital">Initial Capital ($)</label>
        <input
          id="bt-capital"
          v-model.number="form.initial_capital"
          class="input"
          type="number"
          min="0"
          step="100"
        />
      </div>
      <div class="field">
        <label class="label" for="bt-commission">Commission (%)</label>
        <input
          id="bt-commission"
          v-model.number="form.commission"
          class="input"
          type="number"
          min="0"
          max="100"
          step="0.01"
        />
      </div>
    </div>

    <!-- Row 5 -->
    <button
      type="submit"
      class="btn btn-primary run-btn"
      :disabled="!form.strategy_id"
    >
      Run Backtest
    </button>
  </form>
</template>

<style scoped>
.config-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.field {
  display: flex;
  flex-direction: column;
}

.run-btn {
  width: 100%;
}
</style>
