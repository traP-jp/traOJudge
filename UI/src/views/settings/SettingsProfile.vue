<script setup lang="ts">
import PlainTextArea from '@/components/PlainTextArea.vue'
import { MeApi } from '@/api/generated/apis/MeApi'
import { apiErrorMessage, requireEtag } from '@/utils/apiError'
import { useUserStore } from '@/stores/user'
import AlertBox from '@/components/AlertBox.vue'
import PrimaryButton from '@/components/Controls/PrimaryButton.vue'
import PlainTextbox from '@/components/Controls/Textbox/PlainTextbox.vue'
import SnsTextbox from '@/components/Controls/Textbox/SnsTextbox.vue'
import { onMounted, ref } from 'vue'

const displayName = ref<string>('')
const introduction = ref<string>('')
const githubAccount = ref<string>('')
const xAccount = ref<string>('')
const atcoderAccount = ref('')
const etag = ref('')
const userStore = useUserStore()

const displayNameError = ref<string>('')
const introductionError = ref<string>('')
const saveError = ref<string>('')
const saveSuccess = ref<boolean>(false)
const saveErrorShow = ref<boolean>(false)
const isLoading = ref<boolean>(false)

const MAX_INTRO_LENGTH = 4000

async function fetchUserData() {
  isLoading.value = true
  try {
    const meApi = new MeApi()
    const response = await meApi.getMeRaw()
    etag.value = requireEtag(response.raw)
    const user = await response.value()

    displayName.value = user.displayName || ''
    introduction.value = user.bio || ''
    githubAccount.value = user.githubId || ''
    xAccount.value = user.xId || ''
    atcoderAccount.value = user.atcoderId || ''
  } catch (error) {
    console.error('プロフィール情報の取得に失敗しました:', error)
    saveError.value = await apiErrorMessage(error)
    saveErrorShow.value = true
  } finally {
    isLoading.value = false
  }
}

async function saveProfile() {
  saveSuccess.value = false
  saveErrorShow.value = false
  displayNameError.value = ''
  introductionError.value = ''
  saveError.value = ''

  let hasError = false
  if (!displayName.value) {
    displayNameError.value = '表示名は必須です．'
    hasError = true
  }
  if (introduction.value.length > MAX_INTRO_LENGTH) {
    introductionError.value = `自己紹介は${MAX_INTRO_LENGTH}文字以内で入力してください．`
    hasError = true
  }
  if (hasError) return

  isLoading.value = true

  const meApi = new MeApi()
  try {
    const response = await meApi.updateMeRaw({
      origin: window.location.origin,
      ifMatch: etag.value,
      profileUpdate: {
        displayName: displayName.value,
        bio: introduction.value,
        githubId: githubAccount.value,
        xId: xAccount.value,
        atcoderId: atcoderAccount.value
      }
    })
    etag.value = requireEtag(response.raw)
    userStore.user = await response.value()
    saveSuccess.value = true
  } catch (error) {
    console.error('プロフィールの保存に失敗しました:', error)
    saveError.value = await apiErrorMessage(error)
    saveErrorShow.value = true
  } finally {
    isLoading.value = false
  }
}

onMounted(() => {
  fetchUserData()
})
</script>

<template>
  <div class="flex font-primary">
    <div class="flex w-full flex-col">
      <div class="flex max-w-profile-max flex-col items-start justify-start gap-4">
        <h2 class="text-xl font-medium">プロフィール</h2>

        <div class="w-full">
          <PlainTextbox
            id="display-name"
            v-model="displayName"
            class="h-5.5 w-full"
            label="表示名"
            required
            :error-message="displayNameError"
          />
        </div>
        <div class="w-full">
          <PlainTextArea
            id="introduction"
            v-model="introduction"
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
              <SnsTextbox v-model="githubAccount" placeholder="GitHubユーザー名" />
            </div>
          </div>
          <div class="flex items-center gap-2">
            <div class="flex size-6 items-center justify-center">
              <img src="@/assets/service_icons/x.svg" alt="X" class="size-5" />
            </div>
            <div class="flex-1">
              <SnsTextbox v-model="xAccount" placeholder="Xユーザー名" />
            </div>
          </div>
        </div>
        <PlainTextbox v-model="atcoderAccount" label="AtCoderユーザー名" />
        <PrimaryButton
          class="h-10 w-18 px-3 py-2"
          :disabled="isLoading || !etag || displayName.length > 80"
          @click="saveProfile"
        >
          保存
        </PrimaryButton>
        <AlertBox v-model:show="saveSuccess" text="プロフィールが保存されました．" />
        <AlertBox v-model:show="saveErrorShow" :text="saveError" type="error" />
      </div>
    </div>
  </div>
</template>
