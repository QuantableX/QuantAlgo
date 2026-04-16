<script setup lang="ts">
const route = useRoute()

const navItems = [
  { icon: '\u229E', label: 'Dashboard', to: '/' },
  { icon: '\u270E', label: 'Strategies', to: '/strategies' },
  { icon: '\u29C9', label: 'Backtest', to: '/backtest' },
  { icon: '\u25A4', label: 'Terminal', to: '/terminal' },
  { icon: '\u2630', label: 'Journal', to: '/journal' },
  { icon: '\u25E7', label: 'Charts', to: '/charts' },
  { icon: '\u2699', label: 'Exchange', to: '/exchange' },
  { icon: '\u26ED', label: 'Settings', to: '/settings' },
]

function isActive(to: string): boolean {
  if (to === '/') return route.path === '/'
  return route.path.startsWith(to)
}
</script>

<template>
  <nav class="sidebar">
    <div class="nav-items">
      <NuxtLink
        v-for="item in navItems"
        :key="item.to"
        :to="item.to"
        class="nav-item"
        :class="{ active: isActive(item.to) }"
      >
        <span class="nav-icon">{{ item.icon }}</span>
        <span class="nav-label">{{ item.label }}</span>
      </NuxtLink>
    </div>
  </nav>
</template>

<style scoped>
.sidebar {
  width: 220px;
  min-width: 220px;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.nav-items {
  flex: 1;
  padding: 8px 0;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
  margin: 1px 8px;
  border-radius: 6px;
  text-decoration: none;
  color: var(--qa-text-secondary);
  font-size: 13px;
  font-weight: 500;
  transition: background var(--qa-transition), color var(--qa-transition);
}

.nav-item:hover {
  background: var(--qa-bg-hover);
}

.nav-item.active {
  background: var(--qa-bg-hover);
  color: var(--qa-text);
}

.nav-icon {
  width: 20px;
  text-align: center;
  font-size: 15px;
  opacity: 0.5;
  flex-shrink: 0;
}

.nav-item.active .nav-icon {
  opacity: 1;
}

.nav-label {
  white-space: nowrap;
}

</style>
