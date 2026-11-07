<script setup lang="ts">
import { onScopeDispose, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { SubmissionsApi, type JudgeRun, type Submission } from '@/api/generated'
import JudgeRunBadge from '@/components/JudgeRunBadge.vue'
import { apiErrorMessage } from '@/utils/apiError'
import { isJudgeRunFinished, latestJudgeRun } from '@/utils/judgeRun'

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
    run.value = latestJudgeRun(run.value, result)
    error.value = ''
  } catch (cause) {
    const message = await apiErrorMessage(cause)
    if (!signal.aborted) error.value = message
  } finally {
    if (!signal.aborted && !isJudgeRunFinished(run.value))
      timer = setTimeout(() => void load(signal), 2000)
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
    <p v-if="error" class="text-status-error" role="alert">{{ error }} 再取得しています．</p>
    <p v-if="!error && (!submission || !run)">読み込み中...</p>
    <template v-if="submission && run">
      <p>提出日時：{{ submission.submittedAt.toLocaleString('ja-JP') }}</p>
      <JudgeRunBadge :run="run" />
      <template v-if="run.result">
        <p>得点：{{ run.result.score }}</p>
        <p>実行時間：{{ run.result.maxTimeMs }} ms</p>
        <p>メモリ：{{ Math.ceil(run.result.maxMemoryBytes / 1024) }} KiB</p>
      </template>
      <p v-else-if="run.failure" class="text-status-error">{{ run.failure.message }}</p>
    </template>
  </div>
</template>
