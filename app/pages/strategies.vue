<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { useStrategiesStore } from '~/stores/strategies'
import { useBotStore } from '~/stores/bot'
import type { PreflightResult } from '~/types'

const router = useRouter()
const strategiesStore = useStrategiesStore()
const botStore = useBotStore()

// New strategy form state
const showNewForm = ref(false)
const newName = ref('')
const newDesc = ref('')
const isCreating = ref(false)
const isSaving = ref(false)
const saveError = ref<string | null>(null)
const paramsError = ref<string | null>(null)
const isValidating = ref(false)
const validationResult = ref<PreflightResult | null>(null)
const validationError = ref<string | null>(null)

// Deploy modal state
const showDeployModal = ref(false)

const hasActiveStrategy = computed(() => !!strategiesStore.activeStrategyId)
const activeStrategy = computed(() => strategiesStore.activeStrategy)

// Toolbar event handlers
function validateParams(): boolean {
  paramsError.value = null
  const raw = strategiesStore.paramsContent.trim()
  if (!raw) return true
  try {
    JSON.parse(raw)
    return true
  } catch (err) {
    paramsError.value = `Invalid params JSON: ${String(err)}`
    return false
  }
}

async function handleSave(): Promise<boolean> {
  if (!strategiesStore.activeStrategyId) return false
  if (!validateParams()) {
    saveError.value = paramsError.value
    return false
  }
  isSaving.value = true
  saveError.value = null
  try {
    await strategiesStore.save(
      strategiesStore.activeStrategyId,
      strategiesStore.editorContent,
      strategiesStore.paramsContent,
    )
    return true
  } catch (err) {
    saveError.value = String(err)
    console.error('Failed to save strategy:', err)
    return false
  } finally {
    isSaving.value = false
  }
}

async function handleValidate() {
  if (!strategiesStore.activeStrategyId) return
  validationResult.value = null
  validationError.value = null

  const saved = await handleSave()
  if (!saved) {
    validationError.value = saveError.value ?? paramsError.value ?? 'Save the strategy before validation.'
    return
  }

  isValidating.value = true
  try {
    validationResult.value = await invoke<PreflightResult>('validate_strategy', {
      strategyId: strategiesStore.activeStrategyId,
    })
  } catch (err) {
    validationError.value = String(err)
  } finally {
    isValidating.value = false
  }
}

async function handleRename(name: string) {
  if (!strategiesStore.activeStrategyId) return
  const trimmed = name.trim()
  if (!trimmed || trimmed === activeStrategy.value?.name) return

  try {
    await strategiesStore.updateMeta(strategiesStore.activeStrategyId, { name: trimmed })
  } catch (err) {
    console.error('Failed to rename strategy:', err)
  }
}

async function handleDelete() {
  if (!strategiesStore.activeStrategyId || !activeStrategy.value) return
  const confirmed = window.confirm(`Delete "${activeStrategy.value.name}"? This cannot be undone.`)
  if (!confirmed) return

  try {
    await strategiesStore.delete(strategiesStore.activeStrategyId)
  } catch (err) {
    console.error('Failed to delete strategy:', err)
  }
}

function handleRunBacktest() {
  if (!strategiesStore.activeStrategyId) return
  router.push(`/backtest?strategy=${strategiesStore.activeStrategyId}`)
}

function handleDeploy() {
  if (!strategiesStore.activeStrategyId) return
  saveError.value = null
  showDeployModal.value = true
}

async function handleSaveFirst() {
  await handleSave()
  // Modal stays open so refreshed dirty state and preflight can update.
}

function handleDeployStarted() {
  showDeployModal.value = false
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
  // Strategies are loaded in the layout bootstrap, but we still need to
  // select the active strategy to populate the editor
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
      :strategy="activeStrategy"
      :is-saving="isSaving"
      @create="showNewForm = true"
      @remove="handleDelete"
      @save="handleSave"
      @validate="handleValidate"
      @run-backtest="handleRunBacktest"
      @deploy="handleDeploy"
      @stop="botStore.stop()"
      @rename="handleRename"
    />

    <div v-if="showNewForm" class="new-form-wrap">
      <div class="new-form card">
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

    <!-- Editor area -->
    <div v-if="hasActiveStrategy" class="editor-area">
      <StrategyEditor
        v-model:content="strategiesStore.editorContent"
        :strategy-id="strategiesStore.activeStrategyId!"
      />
      <div class="params-panel">
        <div class="params-panel__header">
          <div>
            <h3 class="params-panel__title">Strategy Params</h3>
            <p class="params-panel__hint">Saved JSON is applied before on_start().</p>
          </div>
          <span v-if="strategiesStore.paramsContent !== strategiesStore.savedParamsContent" class="params-panel__dirty">
            Unsaved
          </span>
        </div>
        <textarea
          v-model="strategiesStore.paramsContent"
          class="params-panel__textarea"
          spellcheck="false"
          @blur="validateParams"
        />
        <div v-if="paramsError" class="params-panel__error">{{ paramsError }}</div>
        <div v-if="saveError" class="params-panel__error">{{ saveError }}</div>
        <div v-if="isValidating" class="params-panel__status">Validating strategy...</div>
        <div v-else-if="validationError" class="params-panel__error">{{ validationError }}</div>
        <div v-else-if="validationResult" class="params-panel__checks">
          <div
            v-for="check in validationResult.checks"
            :key="check.id + check.message"
            class="params-panel__check"
            :class="'params-panel__check--' + check.status"
          >
            <span class="params-panel__check-label">{{ check.label }}</span>
            <span class="params-panel__check-message">{{ check.message }}</span>
          </div>
        </div>
      </div>
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
    </div>

    <!-- Deploy Modal (outside v-if/v-else chain, uses Teleport) -->
    <DeployModal
      :visible="showDeployModal"
      :strategy-id="strategiesStore.activeStrategyId"
      :is-dirty="strategiesStore.isDirty"
      :save-error="saveError"
      @close="showDeployModal = false"
      @save-first="handleSaveFirst"
      @started="handleDeployStarted"
    />
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
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
  overflow: hidden;
}

.params-panel {
  flex-shrink: 0;
  border-top: 1px solid var(--qa-border);
  background: var(--qa-bg-card);
  padding: 10px 12px;
}

.params-panel__header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 8px;
}

.params-panel__title {
  font-size: 12px;
  font-weight: 600;
  color: var(--qa-text);
  margin: 0;
}

.params-panel__hint {
  font-size: 11px;
  color: var(--qa-text-muted);
  margin: 2px 0 0;
}

.params-panel__dirty {
  align-self: flex-start;
  color: var(--qa-warning);
  font-size: 11px;
  font-weight: 600;
}

.params-panel__textarea {
  width: 100%;
  min-height: 92px;
  resize: vertical;
  box-sizing: border-box;
  border: 1px solid var(--qa-border);
  border-radius: 6px;
  background: var(--qa-bg-input);
  color: var(--qa-text);
  padding: 8px 10px;
  font-size: 12px;
  line-height: 1.45;
  font-family: ui-monospace, 'Cascadia Code', 'JetBrains Mono', Menlo, Consolas, monospace;
}

.params-panel__textarea:focus {
  outline: none;
  border-color: var(--qa-accent);
}

.params-panel__error,
.params-panel__status {
  margin-top: 6px;
  font-size: 12px;
}

.params-panel__error {
  color: var(--qa-error);
}

.params-panel__status {
  color: var(--qa-text-muted);
}

.params-panel__checks {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 6px;
  margin-top: 8px;
}

.params-panel__check {
  border: 1px solid var(--qa-border);
  border-radius: 6px;
  padding: 6px 8px;
  background: var(--qa-bg-hover);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.params-panel__check--ok {
  border-color: color-mix(in srgb, var(--qa-success) 35%, var(--qa-border));
}

.params-panel__check--warn {
  border-color: color-mix(in srgb, var(--qa-warning) 35%, var(--qa-border));
}

.params-panel__check--error {
  border-color: color-mix(in srgb, var(--qa-error) 35%, var(--qa-border));
}

.params-panel__check-label {
  font-size: 11px;
  color: var(--qa-text);
  font-weight: 600;
}

.params-panel__check-message {
  font-size: 11px;
  color: var(--qa-text-muted);
}

.new-form-wrap {
  padding: 16px;
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
  max-width: 520px;
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
