<script setup lang="ts">
import { ref, watch } from 'vue'
import type { Strategy } from '~/types'

const props = withDefaults(
  defineProps<{
    strategy?: Strategy | null
    isSaving?: boolean
  }>(),
  {
    strategy: null,
    isSaving: false,
  },
)

const emit = defineEmits<{
  save: []
  runBacktest: []
  deploy: []
}>()

const isEditing = ref(false)
const editName = ref('')
const nameInputRef = ref<HTMLInputElement | null>(null)

function startEditing() {
  if (!props.strategy) return
  editName.value = props.strategy.name
  isEditing.value = true
  nextTick(() => {
    nameInputRef.value?.focus()
    nameInputRef.value?.select()
  })
}

function finishEditing() {
  isEditing.value = false
}

watch(
  () => props.strategy?.id,
  () => {
    isEditing.value = false
  },
)
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-left">
      <template v-if="strategy">
        <input
          v-if="isEditing"
          ref="nameInputRef"
          v-model="editName"
          class="name-input"
          type="text"
          @blur="finishEditing"
          @keydown.enter="finishEditing"
          @keydown.escape="finishEditing"
        />
        <button
          v-else
          class="name-display"
          @click="startEditing"
        >
          {{ strategy.name }}
        </button>
      </template>
      <span v-else class="name-placeholder">No strategy selected</span>
    </div>

    <div class="toolbar-right">
      <button
        class="btn btn-sm"
        :disabled="isSaving || !strategy"
        @click="emit('save')"
      >
        <span class="btn-icon">&#10003;</span>
        {{ isSaving ? 'Saving...' : 'Save' }}
      </button>
      <button
        class="btn btn-sm"
        :disabled="!strategy"
        @click="emit('runBacktest')"
      >
        <span class="btn-icon">&#10697;</span>
        Run Backtest
      </button>
      <button
        class="btn btn-sm btn-success"
        :disabled="!strategy"
        @click="emit('deploy')"
      >
        <span class="btn-icon">&#9654;</span>
        Deploy
      </button>
    </div>
  </div>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 44px;
  padding: 0 16px;
  background: var(--qa-bg-card);
  border-bottom: 1px solid var(--qa-border);
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  align-items: center;
  min-width: 0;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.name-display {
  background: none;
  border: none;
  color: var(--qa-text);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  transition: background var(--qa-transition);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 300px;
}

.name-display:hover {
  background: var(--qa-bg-hover);
}

.name-input {
  background: var(--qa-bg-input);
  border: 1px solid var(--qa-accent);
  color: var(--qa-text);
  font-size: 14px;
  font-weight: 600;
  padding: 4px 8px;
  border-radius: 4px;
  outline: none;
  max-width: 300px;
}

.name-placeholder {
  font-size: 14px;
  color: var(--qa-text-muted);
}

.btn-icon {
  font-size: 12px;
  line-height: 1;
}
</style>
