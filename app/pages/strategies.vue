<script setup lang="ts">
import { useStrategiesStore } from '~/stores/strategies'
import { useBotStore } from '~/stores/bot'
import { useExchangeStore } from '~/stores/exchange'

const router = useRouter()
const strategiesStore = useStrategiesStore()
const botStore = useBotStore()
const exchangeStore = useExchangeStore()

// New strategy form state
const showNewForm = ref(false)
const newName = ref('')
const newDesc = ref('')
const isCreating = ref(false)
const isSaving = ref(false)

const hasActiveStrategy = computed(() => !!strategiesStore.activeStrategyId)
const activeStrategy = computed(() => strategiesStore.activeStrategy)

// Toolbar event handlers
async function handleSave() {
  if (!strategiesStore.activeStrategyId) return
  isSaving.value = true
  try {
    const params = activeStrategy.value?.params_json ?? null
    await strategiesStore.save(
      strategiesStore.activeStrategyId,
      strategiesStore.editorContent,
      params,
    )
  } catch (err) {
    console.error('Failed to save strategy:', err)
  } finally {
    isSaving.value = false
  }
}

function handleRunBacktest() {
  if (!strategiesStore.activeStrategyId) return
  router.push(`/backtest?strategy=${strategiesStore.activeStrategyId}`)
}

async function handleDeploy() {
  if (!strategiesStore.activeStrategyId) return
  const exchange = exchangeStore.exchanges[0]
  if (!exchange) {
    console.error('No exchange configured')
    return
  }
  try {
    await botStore.start(
      strategiesStore.activeStrategyId,
      exchange.id,
      'BTC/USDT',
    )
  } catch (err) {
    console.error('Failed to deploy strategy:', err)
  }
}

async function handleCreate() {
  if (!newName.value.trim()) return
  isCreating.value = true
  try {
    const strategy = await strategiesStore.create(newName.value.trim(), newDesc.value.trim())
    await strategiesStore.select(strategy.id)
    showNewForm.value = false
    newName.value = ''
    newDesc.value = ''
  } catch (err) {
    console.error('Failed to create strategy:', err)
  } finally {
    isCreating.value = false
  }
}

function cancelCreate() {
  showNewForm.value = false
  newName.value = ''
  newDesc.value = ''
}

// Keyboard shortcut: Ctrl+S to save
function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault()
    if (hasActiveStrategy.value) {
      handleSave()
    }
  }
}

onMounted(async () => {
  await strategiesStore.load()
  if (strategiesStore.activeStrategyId) {
    await strategiesStore.select(strategiesStore.activeStrategyId)
  }
  window.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <div class="strategies-page">
    <!-- Toolbar -->
    <StrategyToolbar
      @save="handleSave"
      @run-backtest="handleRunBacktest"
      @deploy="handleDeploy"
    />

    <!-- Editor area -->
    <div v-if="hasActiveStrategy" class="editor-area">
      <StrategyEditor
        v-model:content="strategiesStore.editorContent"
        :strategy-id="strategiesStore.activeStrategyId!"
      />
    </div>

    <!-- Empty state -->
    <div v-else class="empty-state">
      <div class="empty-state__content">
        <div class="empty-state__icon">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
            <polyline points="14 2 14 8 20 8" />
            <line x1="12" y1="18" x2="12" y2="12" />
            <line x1="9" y1="15" x2="15" y2="15" />
          </svg>
        </div>
        <p class="empty-state__text">
          Select a strategy from the sidebar or create a new one
        </p>
        <button class="btn btn-primary" @click="showNewForm = true">
          New Strategy
        </button>
      </div>

      <!-- Inline new strategy form -->
      <div v-if="showNewForm" class="new-form card">
        <h3 class="new-form__title">Create New Strategy</h3>
        <div class="new-form__fields">
          <div class="new-form__field">
            <label class="label" for="strategy-name">Name</label>
            <input
              id="strategy-name"
              v-model="newName"
              class="input"
              type="text"
              placeholder="My Strategy"
              @keydown.enter="handleCreate"
            />
          </div>
          <div class="new-form__field">
            <label class="label" for="strategy-desc">Description</label>
            <input
              id="strategy-desc"
              v-model="newDesc"
              class="input"
              type="text"
              placeholder="Brief description..."
              @keydown.enter="handleCreate"
            />
          </div>
        </div>
        <div class="new-form__actions">
          <button class="btn" @click="cancelCreate">Cancel</button>
          <button
            class="btn btn-primary"
            :disabled="!newName.trim() || isCreating"
            @click="handleCreate"
          >
            {{ isCreating ? 'Creating...' : 'Create' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.strategies-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.editor-area {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

/* Empty state */
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 24px;
}

.empty-state__content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.empty-state__icon {
  color: var(--qa-text-muted);
}

.empty-state__text {
  font-size: 14px;
  color: var(--qa-text-muted);
  text-align: center;
  max-width: 320px;
}

/* New strategy form */
.new-form {
  width: 100%;
  max-width: 420px;
}

.new-form__title {
  font-size: 14px;
  font-weight: 600;
  color: var(--qa-text);
  margin-bottom: 16px;
}

.new-form__fields {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 16px;
}

.new-form__field {
  display: flex;
  flex-direction: column;
}

.new-form__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
