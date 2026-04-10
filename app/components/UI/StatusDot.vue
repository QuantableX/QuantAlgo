<script setup lang="ts">
const props = withDefaults(defineProps<{
  status: 'running' | 'stopped' | 'error'
  size?: number
}>(), {
  size: 8,
})

const dotStyle = computed(() => ({
  width: `${props.size}px`,
  height: `${props.size}px`,
}))

const colorVar = computed(() => {
  switch (props.status) {
    case 'running': return 'var(--qa-accent)'
    case 'error': return 'var(--qa-error)'
    default: return 'var(--qa-text-muted)'
  }
})
</script>

<template>
  <span
    class="status-dot"
    :class="{ pulsing: status === 'running' }"
    :style="{ ...dotStyle, backgroundColor: colorVar }"
  />
</template>

<style scoped>
.status-dot {
  display: inline-block;
  border-radius: 50%;
  vertical-align: middle;
  flex-shrink: 0;
}

.status-dot.pulsing {
  animation: pulse-dot 2s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
</style>
