<script setup lang="ts">
import type { Exchange, ExchangeConfig, ExchangeType, ExchangeProvider } from '~/types'

const props = defineProps<{
  exchange?: Exchange
  visible?: boolean
}>()

const emit = defineEmits<{
  submit: [config: ExchangeConfig]
  close: []
  testConnection: []
}>()

const CEX_PROVIDERS: ExchangeProvider[] = ['binance', 'bybit', 'okx', 'coinbase', 'kraken']
const DEX_PROVIDERS: ExchangeProvider[] = ['uniswap', 'jupiter', 'hyperliquid']

const name = ref('')
const exchangeType = ref<ExchangeType>('cex')
const provider = ref<ExchangeProvider>('binance')
const apiKey = ref('')
const apiSecret = ref('')
const passphrase = ref('')
const walletAddress = ref('')
const privateKey = ref('')
const rpcEndpoint = ref('')

const showApiKey = ref(false)
const showApiSecret = ref(false)
const showPrivateKey = ref(false)

const isEditing = computed(() => !!props.exchange)
const modalTitle = computed(() => isEditing.value ? 'Edit Exchange' : 'Add Exchange')

const providerOptions = computed(() =>
  exchangeType.value === 'cex' ? CEX_PROVIDERS : DEX_PROVIDERS
)

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1)
}

function providerLabel(p: ExchangeProvider): string {
  const name = capitalize(p)
  return exchangeType.value === 'dex' ? `${name} (stored credentials only)` : name
}

function resetForm() {
  name.value = ''
  exchangeType.value = 'cex'
  provider.value = 'binance'
  apiKey.value = ''
  apiSecret.value = ''
  passphrase.value = ''
  walletAddress.value = ''
  privateKey.value = ''
  rpcEndpoint.value = ''
  showApiKey.value = false
  showApiSecret.value = false
  showPrivateKey.value = false
}

function prefillForm() {
  if (props.exchange) {
    name.value = props.exchange.name
    exchangeType.value = props.exchange.exchange_type
    provider.value = props.exchange.provider
  } else {
    resetForm()
  }
}

watch(() => props.visible, (val) => {
  if (val !== false) {
    prefillForm()
  }
})

watch(exchangeType, (val) => {
  provider.value = val === 'cex' ? 'binance' : 'uniswap'
})

function onBackdropClick(e: MouseEvent) {
  if ((e.target as HTMLElement).classList.contains('modal-backdrop')) {
    emit('close')
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('close')
  }
}

watch(() => props.visible, (val) => {
  if (val !== false) {
    prefillForm()
    window.addEventListener('keydown', onKeydown)
  } else {
    window.removeEventListener('keydown', onKeydown)
  }
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})

function handleSubmit() {
  const config: ExchangeConfig = {
    name: name.value,
    exchange_type: exchangeType.value,
    provider: provider.value,
  }

  if (exchangeType.value === 'cex') {
    if (apiKey.value) config.api_key = apiKey.value
    if (apiSecret.value) config.api_secret = apiSecret.value
    if (passphrase.value) config.passphrase = passphrase.value
  } else {
    if (walletAddress.value) config.wallet_address = walletAddress.value
    if (privateKey.value) config.private_key = privateKey.value
    if (rpcEndpoint.value) config.rpc_endpoint = rpcEndpoint.value
  }

  emit('submit', config)
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible !== false" class="modal-backdrop" @click="onBackdropClick">
      <div class="modal-content">
        <div class="modal-header">
          <h3 class="modal-title">{{ modalTitle }}</h3>
          <button class="close-btn" @click="emit('close')">&times;</button>
        </div>

        <form class="form-body" @submit.prevent="handleSubmit">
          <!-- Name -->
          <div class="form-group">
            <label class="label">Name</label>
            <input
              v-model="name"
              class="input"
              type="text"
              placeholder="My Exchange"
              required
            />
          </div>

          <!-- Type toggle -->
          <div class="form-group">
            <label class="label">Type</label>
            <div class="type-toggle">
              <button
                type="button"
                class="toggle-option"
                :class="{ active: exchangeType === 'cex' }"
                @click="exchangeType = 'cex'"
              >
                CEX
              </button>
              <button
                type="button"
                class="toggle-option"
                :class="{ active: exchangeType === 'dex' }"
                @click="exchangeType = 'dex'"
              >
                DEX
              </button>
            </div>
          </div>

          <!-- Provider -->
          <div class="form-group">
            <label class="label">Provider</label>
            <select v-model="provider" class="input">
              <option
                v-for="p in providerOptions"
                :key="p"
                :value="p"
              >
                {{ providerLabel(p) }}
              </option>
            </select>
            <p v-if="exchangeType === 'dex'" class="form-note">
              DEX credentials can be stored here, but deploy and pair discovery are not supported yet.
            </p>
          </div>

          <!-- CEX fields -->
          <template v-if="exchangeType === 'cex'">
            <div class="form-group">
              <label class="label">API Key</label>
              <div class="input-reveal">
                <input
                  v-model="apiKey"
                  class="input"
                  :type="showApiKey ? 'text' : 'password'"
                  placeholder="Enter API key"
                  autocomplete="off"
                />
                <button
                  type="button"
                  class="reveal-btn"
                  @click="showApiKey = !showApiKey"
                >
                  {{ showApiKey ? 'Hide' : 'Show' }}
                </button>
              </div>
            </div>

            <div class="form-group">
              <label class="label">API Secret</label>
              <div class="input-reveal">
                <input
                  v-model="apiSecret"
                  class="input"
                  :type="showApiSecret ? 'text' : 'password'"
                  placeholder="Enter API secret"
                  autocomplete="off"
                />
                <button
                  type="button"
                  class="reveal-btn"
                  @click="showApiSecret = !showApiSecret"
                >
                  {{ showApiSecret ? 'Hide' : 'Show' }}
                </button>
              </div>
            </div>

            <div class="form-group">
              <label class="label">Passphrase <span class="optional">(optional)</span></label>
              <input
                v-model="passphrase"
                class="input"
                type="password"
                placeholder="Exchange passphrase"
                autocomplete="off"
              />
            </div>
          </template>

          <!-- DEX fields -->
          <template v-if="exchangeType === 'dex'">
            <div class="form-group">
              <label class="label">Wallet Address</label>
              <input
                v-model="walletAddress"
                class="input"
                type="text"
                placeholder="0x..."
                autocomplete="off"
              />
            </div>

            <div class="form-group">
              <label class="label">Private Key</label>
              <div class="input-reveal">
                <input
                  v-model="privateKey"
                  class="input"
                  :type="showPrivateKey ? 'text' : 'password'"
                  placeholder="Enter private key"
                  autocomplete="off"
                />
                <button
                  type="button"
                  class="reveal-btn"
                  @click="showPrivateKey = !showPrivateKey"
                >
                  {{ showPrivateKey ? 'Hide' : 'Show' }}
                </button>
              </div>
            </div>

            <div class="form-group">
              <label class="label">RPC Endpoint <span class="optional">(optional)</span></label>
              <input
                v-model="rpcEndpoint"
                class="input"
                type="text"
                placeholder="https://..."
                autocomplete="off"
              />
            </div>
          </template>

          <!-- Actions -->
          <div class="form-actions">
            <button type="button" class="btn" @click="emit('testConnection')">
              Test Connection
            </button>
            <div class="actions-right">
              <button type="button" class="btn" @click="emit('close')">Cancel</button>
              <button type="submit" class="btn btn-primary">Save</button>
            </div>
          </div>
        </form>
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
  max-width: 500px;
  width: 90%;
  max-height: 85vh;
  overflow-y: auto;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}

.modal-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--qa-text);
}

.close-btn {
  background: none;
  border: none;
  color: var(--qa-text-muted);
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
  transition: all var(--qa-transition);
}

.close-btn:hover {
  color: var(--qa-text);
  background: var(--qa-bg-hover);
}

.form-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.optional {
  font-weight: 400;
  color: var(--qa-text-muted);
  text-transform: none;
  letter-spacing: 0;
}

.form-note {
  margin: 4px 0 0;
  font-size: 12px;
  line-height: 1.4;
  color: var(--qa-warning);
}

.type-toggle {
  display: flex;
  border: 1px solid var(--qa-border);
  border-radius: var(--qa-radius);
  overflow: hidden;
}

.toggle-option {
  flex: 1;
  padding: 8px;
  font-size: 13px;
  font-weight: 600;
  text-align: center;
  background: var(--qa-bg-input);
  color: var(--qa-text-muted);
  border: none;
  cursor: pointer;
  transition: all var(--qa-transition);
}

.toggle-option:first-child {
  border-right: 1px solid var(--qa-border);
}

.toggle-option.active {
  background: var(--qa-bg-hover);
  color: var(--qa-text);
}

.toggle-option:hover:not(.active) {
  background: var(--qa-bg-hover);
}

.input-reveal {
  position: relative;
  display: flex;
  align-items: center;
}

.input-reveal .input {
  padding-right: 60px;
}

.reveal-btn {
  position: absolute;
  right: 8px;
  background: none;
  border: none;
  color: var(--qa-text-muted);
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 3px;
  transition: color var(--qa-transition);
}

.reveal-btn:hover {
  color: var(--qa-text);
}

.form-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 8px;
}

.actions-right {
  display: flex;
  gap: 8px;
}
</style>
