import tailwindcss from '@tailwindcss/vite'
import pkg from './package.json'

export default defineNuxtConfig({
  ssr: false,
  devtools: { enabled: false },

  runtimeConfig: {
    public: {
      appVersion: pkg.version,
    },
  },

  srcDir: 'app',

  modules: [
    '@pinia/nuxt',
    '@vueuse/nuxt',
  ],

  css: [
    '~/assets/css/main.css',
  ],

  app: {
    head: {
      title: 'QuantAlgo',
      meta: [
        { name: 'description', content: 'Crypto Trading Bot Terminal' },
      ],
    },
  },

  vite: {
    plugins: [tailwindcss()],
    optimizeDeps: {
      include: ['monaco-editor', 'lightweight-charts'],
    },
    clearScreen: false,
    envPrefix: ['VITE_', 'TAURI_'],
  },

  devServer: {
    port: 1420,
  },

  compatibilityDate: '2025-01-01',
})
