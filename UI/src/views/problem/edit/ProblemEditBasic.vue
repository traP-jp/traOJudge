<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { ProblemsApi, type Problem } from '@/api/generated'
import PlainTextbox from '@/components/Controls/Textbox/PlainTextbox.vue'
import PrimaryButton from '@/components/Controls/PrimaryButton.vue'
import BorderedButton from '@/components/Controls/BorderedButton.vue'
import { useMockAuthGuard } from '@/composables/useMockAuthGuard'
import { useUserStore } from '@/stores/user'
import { apiErrorMessage, requireEtag } from '@/utils/apiError'
const userStore = useUserStore()
const canUnpublish = computed(() =>
  userStore.user?.roles.some(
    (role) => role.role === 'service-admin' && role.scope.kind === 'service'
  )
)
const route = useRoute()
const api = new ProblemsApi()
const { requireMockLogin } = useMockAuthGuard()
const problem = ref<Problem>()
const title = ref('')
const etag = ref('')
const error = ref('')
const message = ref('')
const busy = ref(false)
watch(
  () => route.params.id,
  async (id) => {
    problem.value = undefined
    etag.value = ''
    try {
      const response = await api.getProblemRaw({ problemId: String(id) })
      etag.value = requireEtag(response.raw)
      problem.value = await response.value()
      title.value = problem.value.title
    } catch (cause) {
      error.value = await apiErrorMessage(cause)
    }
  },
  { immediate: true }
)
async function save() {
  if (!problem.value) return
  busy.value = true
  error.value = ''
  message.value = ''
  try {
    const response = await requireMockLogin(() =>
      api.updateProblemRaw({
        origin: window.location.origin,
        ifMatch: etag.value,
        problemId: String(route.params.id),
        problemUpdate: { title: title.value }
      })
    )
    etag.value = requireEtag(response.raw)
    problem.value = await response.value()
    message.value = '保存しました．'
  } catch (cause) {
    error.value = await apiErrorMessage(cause)
  } finally {
    busy.value = false
  }
}
async function unpublish() {
  busy.value = true
  error.value = ''
  try {
    await requireMockLogin(() =>
      api.unpublishProblem({ origin: window.location.origin, problemId: String(route.params.id) })
    )
    const response = await api.getProblemRaw({ problemId: String(route.params.id) })
    etag.value = requireEtag(response.raw)
    problem.value = await response.value()
    message.value = '非公開にしました．'
  } catch (cause) {
    error.value = await apiErrorMessage(cause)
  } finally {
    busy.value = false
  }
}
</script>
<template>
  <div class="flex w-full max-w-profile-max flex-col gap-4">
    <h2 class="fontstyle-ui-subtitle">基本設定</h2>
    <PlainTextbox v-model="title" label="問題タイトル" required />
    <p>公開状態：{{ problem?.visibility === 'public' ? '公開' : '非公開' }}</p>
    <p class="text-text-secondary">公開への変更は準備中です．</p>
    <BorderedButton
      v-if="problem?.visibility === 'public' && canUnpublish"
      :disabled="busy"
      @click="unpublish"
      >非公開にする</BorderedButton
    >
    <PrimaryButton :disabled="busy || !etag || !title.trim() || title.length > 200" @click="save"
      >保存</PrimaryButton
    >
    <p v-if="message" role="status">{{ message }}</p>
    <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
  </div>
</template>
