<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import { type Problem, ProblemsApi } from '@/api/generated'
import AlertBox from '@/components/AlertBox.vue'
import PlainTextArea from '@/components/controls/PlainTextArea.vue'
import PrimaryButton from '@/components/controls/PrimaryButton.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { useMockAuthGuard } from '@/composables/problems/useMockAuthGuard'

const route = useRoute()
const { requireMockLogin } = useMockAuthGuard()

const problem = ref<Problem>()
const body = ref<string>('')

const saveError = ref<string>('')
const saveSuccess = ref<boolean>(false)
const saveErrorShow = ref<boolean>(false)
const isLoading = ref<boolean>(false)

if (typeof route.params.id !== 'string') throw new Error('Invalid route')
const problemId = ref<string>('')
watch(
  () => route.params.id,
  (id) => {
    problemId.value = String(id)
  },
  { immediate: true }
)

const problemsApi = new ProblemsApi()

async function fetchProblem() {
  isLoading.value = true
  try {
    problem.value = await problemsApi.getProblem({ problemId: problemId.value })
    body.value = problem.value.statement
  } catch (error) {
    console.error('問題の取得に失敗しました:', error)
    saveError.value = '問題の取得に失敗しました。'
    saveErrorShow.value = true
  } finally {
    isLoading.value = false
  }
}

async function saveStatement() {
  if (problem.value == null) return
  isLoading.value = true
  try {
    problem.value.statement = body.value
    await requireMockLogin(() =>
      problemsApi.putProblem({
        problemId: problemId.value,
        putProblemRequest: problem.value!
      })
    )
    saveSuccess.value = true
  } catch (error) {
    console.error('問題文の保存に失敗しました:', error)
    saveError.value = error instanceof Error ? error.message : '問題文の保存に失敗しました。'
    saveErrorShow.value = true
  } finally {
    isLoading.value = false
  }
}

onMounted(fetchProblem)
</script>

<template>
  <div class="flex font-primary">
    <div class="flex w-full flex-col">
      <div class="flex w-full flex-col items-start justify-start gap-4">
        <h2 class="text-xl font-medium">問題文</h2>

        <div class="grid w-full grid-cols-1 gap-4 lg:grid-cols-2">
          <div class="flex min-w-0 flex-col gap-1">
            <label for="problem-statement" class="fontstyle-ui-body-2 text-text-secondary">
              入力 (Markdown)
            </label>
            <PlainTextArea id="problem-statement" v-model="body" class="h-96 w-full" required />
          </div>
          <div class="flex min-w-0 flex-col gap-1">
            <span class="fontstyle-ui-body-2 text-text-secondary">プレビュー</span>
            <div
              class="h-96 w-full overflow-auto rounded-sm border border-border-primary bg-background-primary p-4"
            >
              <MarkdownContent v-if="body" :source="body" />
              <p v-else class="fontstyle-ui-body text-text-tertiary">
                入力するとプレビューが表示されます
              </p>
            </div>
          </div>
        </div>

        <PrimaryButton class="h-10 w-18 px-3 py-2" :disabled="isLoading" @click="saveStatement">
          保存
        </PrimaryButton>

        <AlertBox v-model:show="saveSuccess" text="問題文が保存されました。" />
        <AlertBox v-model:show="saveErrorShow" :text="saveError" type="error" />
      </div>
    </div>
  </div>
</template>
