<script setup lang="ts">
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import { useJudgeRunQuery } from '@/composables/submissions/useJudgeRunQuery'
import { isJudgeRunFinished, judgeRunStateLabels } from '@/utils/judgeRun'

const props = defineProps<{ submissionId: string; judgeRunId: string; canRead: boolean }>()
const { view, refresh } = useJudgeRunQuery(
  () => ({ submissionId: props.submissionId, judgeRunId: props.judgeRunId }),
  () => props.canRead
)
</script>

<template>
  <span v-if="!canRead" class="text-text-secondary">非公開</span>
  <span v-else-if="view.status === 'pending'" class="text-text-secondary">読み込み中...</span>
  <div v-else-if="view.status === 'error'" class="space-y-1 text-sm">
    <p>{{ view.message }}</p>
    <MonochromeButton small @click="refresh">再試行</MonochromeButton>
  </div>
  <div v-else class="space-y-1">
    <span
      class="inline-block rounded-sm px-2 font-semibold text-white"
      :class="
        view.data.result?.verdict === 'AC'
          ? 'bg-status-accepted'
          : isJudgeRunFinished(view.data)
            ? 'bg-status-failed'
            : 'bg-status-pending'
      "
      >{{ view.data.result?.verdict ?? judgeRunStateLabels[view.data.state] }}</span
    >
    <p v-if="!isJudgeRunFinished(view.data)" class="text-sm text-text-secondary">
      {{ view.data.completedCaseCount }} / {{ view.data.totalCaseCount ?? '—' }} ケース
    </p>
    <p v-if="view.data.result" class="text-sm tabular-nums">
      {{ view.data.result.score }} 点 · {{ view.data.result.maxTimeMs }} ms ·
      {{ (view.data.result.maxMemoryBytes / 1024 / 1024).toFixed(1) }} MiB
    </p>
    <div v-if="view.warning" class="text-sm text-status-error">
      <p>{{ view.warning }}</p>
      <MonochromeButton small @click="refresh">再試行</MonochromeButton>
    </div>
  </div>
</template>
