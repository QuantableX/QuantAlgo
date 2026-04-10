<script setup lang="ts">
withDefaults(defineProps<{
  label: string
  value: string
  variant?: 'default' | 'success' | 'error' | 'warning'
}>(), {
  variant: 'default',
})
</script>

<template>
  <span class="metric-pill pill">
    <span class="metric-label">{{ label }}</span>
    <span
      class="metric-value"
      :class="{
        'text-success': variant === 'success',
        'text-error': variant === 'error',
        'text-warning': variant === 'warning',
      }"
    >{{ value }}</span>
  </span>
</template>

<style scoped>
.metric-pill {
  gap: 6px;
}

.metric-label {
  color: var(--qa-text-muted);
}

.metric-value {
  color: var(--qa-text);
  font-weight: 600;
}

.metric-value.text-success {
  color: var(--qa-success);
}

.metric-value.text-error {
  color: var(--qa-error);
}

.metric-value.text-warning {
  color: var(--qa-warning);
}
</style>
