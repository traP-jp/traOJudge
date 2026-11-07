<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { ProblemsApi } from '@/api/generated'
import PlainTextbox from '@/components/Controls/Textbox/PlainTextbox.vue'
import PlainTextArea from '@/components/PlainTextArea.vue'
import PrimaryButton from '@/components/Controls/PrimaryButton.vue'
import { useMockAuthGuard } from '@/composables/useMockAuthGuard'
import { apiErrorMessage } from '@/utils/apiError'
const router = useRouter()
const { requireMockLogin } = useMockAuthGuard()
const title = ref('')
const statement = ref('')
const error = ref('')
const saving = ref(false)
async function createProblem() {
  saving.value = true
  error.value = ''
  try {
    const problem = await requireMockLogin(() =>
      new ProblemsApi().createProblem({
        origin: window.location.origin,
        idempotencyKey: crypto.randomUUID(),
        problemCreate: { title: title.value, statement: statement.value }
      })
    )
    await router.push(`/problems/${problem.id}`)
  } catch (cause) {
    error.value = await apiErrorMessage(cause)
  } finally {
    saving.value = false
  }
}
</script>
<template>
  <div class="flex w-full justify-center px-container-x py-6">
    <div class="flex w-full max-w-profile-max flex-col gap-4">
      <h2 class="fontstyle-ui-subtitle">問題の新規作成</h2>
      <PlainTextbox v-model="title" label="問題タイトル" required />
      <PlainTextArea v-model="statement" label="問題文（Markdown）" class="h-64" />
      <PrimaryButton
        class="h-12 shrink-0"
        :disabled="saving || !title.trim() || title.length > 200 || statement.length > 200000"
        @click="createProblem"
        >作成</PrimaryButton
      >
      <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
    </div>
  </div>
</template>
