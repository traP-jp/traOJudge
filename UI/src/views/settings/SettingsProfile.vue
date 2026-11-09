<script setup lang="ts">
import { computed, onScopeDispose, ref, watch } from 'vue'

import { apiClient, type CurrentUser, requireData } from '@/api/client'
import { ApiError } from '@/api/errors'
import { queryClient } from '@/api/queryClient'
import AlertBox from '@/components/AlertBox.vue'
import PlainTextArea from '@/components/controls/PlainTextArea.vue'
import PrimaryButton from '@/components/controls/PrimaryButton.vue'
import PlainTextbox from '@/components/controls/textbox/PlainTextbox.vue'
import SnsTextbox from '@/components/controls/textbox/SnsTextbox.vue'
import { useUserStore } from '@/stores/user'

const session = useUserStore()
const emptyProfile = () => ({ displayName: '', bio: '', githubId: '', atcoderId: '', xId: '' })
const form = ref(emptyProfile())
const displayNameError = ref('')
const introductionError = ref('')
const saveError = ref('')
const saveSuccess = ref(false)
const saveErrorShow = ref(false)
const isLoading = ref(false)
const etag = ref<string | null>(null)
const canSave = computed(() => session.isAuthenticated && !!etag.value && !isLoading.value)
let controller: AbortController | undefined

const setFields = (user: CurrentUser) => {
  const { displayName, bio, githubId, atcoderId, xId } = user
  form.value = { displayName, bio, githubId, atcoderId, xId }
}
const beginRequest = () => {
  controller?.abort()
  controller = new AbortController()
  const { signal } = controller
  const epoch = session.epoch
  const userId = session.userId
  isLoading.value = true
  saveErrorShow.value = false
  saveSuccess.value = false
  return {
    signal,
    isCurrent: () => !signal.aborted && epoch === session.epoch && userId === session.userId
  }
}
const showError = (error: unknown, message: string) => {
  if (error instanceof ApiError && error.response.status === 401) {
    session.reset()
    return
  }
  saveError.value = message
  saveErrorShow.value = true
}

const fetchUserData = async () => {
  if (!session.isAuthenticated) return
  const operation = beginRequest()
  etag.value = null
  try {
    const { data, response } = await apiClient.GET('/me', { signal: operation.signal })
    if (!operation.isCurrent()) return
    const user = requireData(data)
    if (user.id !== session.userId) {
      await session.fetchCurrentUser()
      return
    }
    const version = response.headers.get('ETag')
    if (!version) throw new Error('Missing profile ETag')
    setFields(user)
    etag.value = version
    displayNameError.value = ''
    introductionError.value = ''
  } catch (error) {
    if (operation.isCurrent()) showError(error, 'プロフィール情報の取得に失敗しました。')
  } finally {
    if (operation.isCurrent()) isLoading.value = false
  }
}

const saveProfile = async () => {
  if (!canSave.value || !etag.value) return
  displayNameError.value = ''
  introductionError.value = ''
  saveSuccess.value = false
  saveErrorShow.value = false
  if (
    Array.from(form.value.displayName).length < 1 ||
    Array.from(form.value.displayName).length > 80
  )
    displayNameError.value = '表示名は1〜80文字で入力してください。'
  if (Array.from(form.value.bio).length > 4000)
    introductionError.value = '自己紹介は4000文字以内で入力してください。'
  if (displayNameError.value || introductionError.value) return

  const operation = beginRequest()
  try {
    const { data, response } = await apiClient.PATCH('/me', {
      params: { header: { 'If-Match': etag.value, Origin: window.location.origin } },
      body: { ...form.value },
      signal: operation.signal
    })
    if (!operation.isCurrent()) return
    const user = requireData(data)
    if (user.id !== session.userId) {
      await session.fetchCurrentUser()
      return
    }
    session.user = user
    setFields(user)
    etag.value = response.headers.get('ETag')
    saveSuccess.value = true
    void queryClient.invalidateQueries({
      queryKey: ['user']
    })
  } catch (error) {
    if (!operation.isCurrent()) return
    if (error instanceof ApiError && error.response.status === 412) {
      etag.value = null
      showError(
        error,
        'ほかの操作でプロフィールが更新されました。入力内容を控えてから、最新の内容を読み込み直してください。'
      )
      return
    }
    showError(
      error,
      'プロフィールの保存に失敗しました。入力内容を確認して、もう一度お試しください。'
    )
  } finally {
    if (operation.isCurrent()) isLoading.value = false
  }
}

watch(
  () => [session.epoch, session.isAuthenticated],
  () => {
    controller?.abort()
    form.value = emptyProfile()
    etag.value = null
    isLoading.value = false
    saveSuccess.value = false
    saveErrorShow.value = false
    displayNameError.value = ''
    introductionError.value = ''
    void fetchUserData()
  },
  { immediate: true, flush: 'sync' }
)
onScopeDispose(() => controller?.abort())
</script>

<template>
  <div class="flex font-primary">
    <div class="flex w-full flex-col">
      <fieldset
        :disabled="isLoading || !session.isAuthenticated"
        class="flex max-w-profile-max flex-col items-start justify-start gap-4"
      >
        <h2 class="text-xl font-medium">プロフィール</h2>

        <div class="w-full">
          <PlainTextbox
            id="display-name"
            v-model="form.displayName"
            class="h-5.5 w-full"
            label="表示名"
            required
            :error-message="displayNameError"
          />
        </div>
        <div class="w-full">
          <PlainTextArea
            id="introduction"
            v-model="form.bio"
            class="h-27 w-full"
            label="自己紹介"
            :error-message="introductionError"
            :displays-length="true"
          />
        </div>
        <div class="flex w-full max-w-profile-max flex-col gap-1">
          <div class="flex items-center gap-2">
            <span class="fontstyle-ui-control text-text-primary">SNSアカウント</span>
          </div>
          <div class="flex items-center gap-2">
            <div class="flex size-6 items-center justify-center">
              <img src="@/assets/service_icons/github.svg" alt="GitHub" class="size-5" />
            </div>
            <div class="flex-1">
              <SnsTextbox
                v-model="form.githubId"
                placeholder="GitHubユーザー名"
                aria-label="GitHubユーザー名"
              />
            </div>
          </div>
          <div class="flex items-center gap-2">
            <div class="flex size-6 items-center justify-center">
              <img src="@/assets/service_icons/x.svg" alt="X" class="size-5" />
            </div>
            <div class="flex-1">
              <SnsTextbox v-model="form.xId" placeholder="Xユーザー名" aria-label="Xユーザー名" />
            </div>
          </div>
          <div class="flex items-center gap-2">
            <span class="fontstyle-ui-control">AtCoder</span>
            <div class="flex-1">
              <SnsTextbox
                v-model="form.atcoderId"
                placeholder="AtCoderユーザー名"
                aria-label="AtCoderユーザー名"
              />
            </div>
          </div>
        </div>
        <PrimaryButton class="h-10 w-18 px-3 py-2" :disabled="!canSave" @click="saveProfile">
          保存
        </PrimaryButton>
        <PrimaryButton v-if="!etag && !isLoading" class="px-3 py-2" @click="fetchUserData">
          最新の内容を読み込む（入力を置き換えます）
        </PrimaryButton>
        <AlertBox v-model:show="saveSuccess" text="プロフィールが保存されました。" />
        <AlertBox v-model:show="saveErrorShow" :text="saveError" type="error" />
      </fieldset>
    </div>
  </div>
</template>
