<script setup lang="ts">
import { responseErrorMessage } from '@/api/errors'
import { ResponseError } from '@/api/generated/runtime'
import { useOAuthStore } from '@/stores/oauth'
import type { OAuthProvider } from '@/types/oauth'

const {
  disabled = false,
  app,
  action
} = defineProps<{
  disabled?: boolean
  app: string
  action: 'signup' | 'login'
}>()

const oauthStore = useOAuthStore()

async function onOAuthClick() {
  try {
    if (app === 'GitHub' || app === 'Google' || app === 'traQ') {
      const provider = app.toLowerCase() as OAuthProvider
      await oauthStore.initiateOAuth(provider, action)
    } else {
      throw new Error('Unknown OAuth app: ' + app)
    }
  } catch (error: unknown) {
    if (!(error instanceof ResponseError)) {
      console.error('OAuth Error:', error)
      return
    }
    if (error.response.status === 500) {
      const message = await responseErrorMessage(error.response)
      console.error('Internal Server Error: ' + message)
    } else {
      console.error('Unknown error: ' + error.response.status)
    }
  }
}
</script>

<template>
  <button
    :disabled="disabled"
    class="inline-flex items-center justify-center gap-2.5 rounded-lg border border-border-secondary px-3 py-2 fontstyle-ui-control-strong text-text-primary enabled:hover:bg-background-secondary disabled:opacity-50"
    @click="onOAuthClick"
  >
    <span v-if="app === 'GitHub'" class="inline-block align-middle">
      <img src="/src/assets/service_icons/github.svg" alt="" class="size-5" />
    </span>
    <span v-if="app === 'Google'" class="inline-block align-middle"
      ><img src="/src/assets/service_icons/google.svg" alt="" class="size-5"
    /></span>
    <span v-if="app === 'traQ'" class="inline-block align-middle"
      ><img src="/src/assets/service_icons/traq.svg" alt="" class="size-5"
    /></span>
    <span class="inline-block align-middle"
      >{{ app }} で{{ action === 'signup' ? '新規登録' : 'ログイン' }}</span
    >
  </button>
</template>
