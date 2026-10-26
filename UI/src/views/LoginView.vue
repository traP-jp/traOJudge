<script setup lang="ts">
import PrimaryButton from '@/components/Controls/PrimaryButton.vue'
import { useUserStore } from '@/stores/user'
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()
const userStore = useUserStore()
const mockError = ref('')
const canUseMock = import.meta.env.DEV

const loginWithMock = async () => {
  mockError.value = ''
  try {
    await userStore.loginWithMock()
    const redirect = route.query.redirect
    await router.push(typeof redirect === 'string' && redirect.startsWith('/') ? redirect : '/')
  } catch {
    mockError.value = 'モックでログインできませんでした．モックサーバーと設定を確認してください．'
  }
}
</script>

<template>
  <div class="flex h-header-offset items-start justify-center bg-background-secondary p-6">
    <div class="flex w-auth-form flex-col gap-6 rounded-15 bg-background-primary px-8 py-6">
      <h1 class="fontstyle-ui-subtitle text-text-primary">ログイン</h1>
      <p class="fontstyle-ui-body text-text-secondary">
        メールと外部サービスによるログインは準備中です．
      </p>
      <template v-if="canUseMock">
        <p class="fontstyle-ui-body text-text-secondary">
          開発中はモックサーバーを起動すると，仮の利用者としてログインできます．
        </p>
        <PrimaryButton
          class="h-10 w-full px-4 py-3"
          :disabled="userStore.isLoading"
          @click="loginWithMock"
        >
          {{ userStore.isLoading ? 'ログイン中...' : 'モックでログイン' }}
        </PrimaryButton>
        <p v-if="mockError" role="alert" class="fontstyle-ui-caption text-status-error">
          {{ mockError }}
        </p>
      </template>
    </div>
  </div>
</template>
