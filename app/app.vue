<script setup lang="ts">
import { useAppStore } from '~/stores/app'

const app = useAppStore()

onMounted(() => {
  app.loadSettings()
})
</script>

<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>
