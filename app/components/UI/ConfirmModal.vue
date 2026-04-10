<script setup lang="ts">
const props = withDefaults(defineProps<{
  visible: boolean
  title: string
  message: string
  confirmText?: string
  cancelText?: string
  variant?: 'danger' | 'default'
}>(), {
  confirmText: 'Confirm',
  cancelText: 'Cancel',
  variant: 'default',
})

const emit = defineEmits<{
  confirm: []
  cancel: []
}>()

function onBackdropClick(e: MouseEvent) {
  if ((e.target as HTMLElement).classList.contains('modal-backdrop')) {
    emit('cancel')
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('cancel')
  }
}

watch(() => props.visible, (val) => {
  if (val) {
    window.addEventListener('keydown', onKeydown)
  } else {
    window.removeEventListener('keydown', onKeydown)
  }
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop" @click="onBackdropClick">
      <div class="modal-content">
        <h3 class="modal-title">{{ title }}</h3>
        <p class="modal-message">{{ message }}</p>
        <div class="modal-actions">
          <button class="btn" @click="emit('cancel')">{{ cancelText }}</button>
          <button
            class="btn"
            :class="variant === 'danger' ? 'btn-danger' : 'btn-primary'"
            @click="emit('confirm')"
          >
            {{ confirmText }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-content {
  background: var(--qa-bg-card);
  border: 1px solid var(--qa-border);
  border-radius: var(--qa-radius-lg);
  padding: 24px;
  max-width: 420px;
  width: 90%;
}

.modal-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--qa-text);
  margin-bottom: 8px;
}

.modal-message {
  font-size: 14px;
  color: var(--qa-text-secondary);
  line-height: 1.5;
  margin-bottom: 20px;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
