import tailwindcss from '@tailwindcss/vite'
import pkg from './package.json'

const appManifestStubUrl = new URL('./app/shims/app-manifest.ts', import.meta.url).pathname
const appManifestStub = decodeURIComponent(
  /^\/[A-Za-z]:/.test(appManifestStubUrl) ? appManifestStubUrl.slice(1) : appManifestStubUrl,
)

export default defineNuxtConfig({
  ssr: false,
  devtools: { enabled: false },

  experimental: {
    appManifest: false,
  },

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

  alias: {
    '#app-manifest': appManifestStub,
  },

  vite: {
    plugins: [tailwindcss()],
    resolve: {
      alias: {
        '#app-manifest': appManifestStub,
      },
    },
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
