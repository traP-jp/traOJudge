<script setup lang="ts">
import { onScopeDispose, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { SubmissionsApi, type JudgeRun, type Submission } from '@/api/generated'
import JudgeResultBadge from '@/components/JudgeResultBadge.vue'
import { apiErrorMessage } from '@/utils/apiError'

const route = useRoute()
const api = new SubmissionsApi()
const submission = ref<Submission>()
const run = ref<JudgeRun>()
const error = ref('')
let timer: ReturnType<typeof setTimeout> | undefined
let controller: AbortController | undefined
async function load(signal: AbortSignal) {
  try {
    const item = await api.getSubmission(
      { submissionId: String(route.params.submissionId) },
      { signal }
    )
    const result = await api.getJudgeRun(
      { submissionId: item.id, judgeRunId: item.currentJudgeRunId },
      { signal }
    )
    if (signal.aborted) return
    submission.value = item
    run.value = result
    if (result.state !== 'completed' && result.state !== 'failed')
      timer = setTimeout(() => void load(signal), 2000)
  } catch (cause) {
    if (!signal.aborted) error.value = await apiErrorMessage(cause)
  }
}
watch(
  () => route.params.submissionId,
  () => {
    clearTimeout(timer)
    controller?.abort()
    controller = new AbortController()
    submission.value = undefined
    run.value = undefined
    error.value = ''
    void load(controller.signal)
  },
  { immediate: true }
)
onScopeDispose(() => {
  clearTimeout(timer)
  controller?.abort()
})
</script>

<template>
  <div class="space-y-4">
    <h2 class="fontstyle-ui-subtitle">提出結果</h2>
    <p v-if="error" class="text-status-error" role="alert">{{ error }}</p>
    <p v-else-if="!submission || !run">読み込み中...</p>
    <template v-else>
      <p>提出日時：{{ submission.submittedAt.toLocaleString('ja-JP') }}</p>
      <template v-if="run.result">
        <JudgeResultBadge :status="run.result.verdict" />
        <p>得点：{{ run.result.score }}</p>
        <p>実行時間：{{ run.result.maxTimeMs }} ms</p>
        <p>メモリ：{{ Math.ceil(run.result.maxMemoryBytes / 1024) }} KiB</p>
      </template>
      <p v-else-if="run.failure" class="text-status-error">{{ run.failure.message }}</p>
      <p v-else>
        採点中：{{ run.completedCaseCount }} / {{ run.totalCaseCount ?? '未確定' }} ケース
      </p>
    </template>
  </div>
</template>
