<script setup lang="ts">
import { computed } from 'vue'
import type { JudgeRun } from '@/api/generated'
import JudgeResultBadge from '@/components/JudgeResultBadge.vue'
import { isJudgeRunFinished, judgeRunStateLabels } from '@/utils/judgeRun'

const { run } = defineProps<{ run: JudgeRun }>()
const progress = computed(() => {
  if (isJudgeRunFinished(run)) return undefined
  return {
    completed: run.completedCaseCount,
    total: typeof run.totalCaseCount === 'number' ? run.totalCaseCount : null
  }
})
</script>

<template>
  <span
    v-if="run.state === 'failed'"
    role="status"
    class="fontstyle-ui-control-strong inline-flex whitespace-nowrap rounded bg-status-error px-2 text-white"
    >採点失敗</span
  >
  <JudgeResultBadge
    v-else
    :status="run.result?.verdict ?? 'WJ'"
    :progress="progress"
    :title="judgeRunStateLabels[run.state]"
  />
</template>
