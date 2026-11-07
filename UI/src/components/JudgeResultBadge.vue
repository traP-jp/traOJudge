<script setup lang="ts">
import { computed } from 'vue'
import type { JudgeStatus } from '@/utils/statusConfig'
import { statusConfig } from '@/utils/statusConfig'

const props = defineProps<{
  status: JudgeStatus
  progress?: { completed: number; total: number | null }
}>()

const config = computed(() => {
  const status = props.progress && props.status === 'AC' ? 'WJ' : props.status
  return statusConfig[status] || statusConfig.WJ
})
const label = computed(() => {
  if (!props.progress) return config.value.label
  return `${config.value.label} ${props.progress.completed}/${props.progress.total ?? '?'}`
})
const accessibleLabel = computed(() => {
  if (!props.progress) return config.value.label
  return `${config.value.label}，${props.progress.completed} / ${props.progress.total ?? '総数不明'} ケース完了，採点中`
})
</script>

<template>
  <div
    class="fontstyle-ui-control-strong inline-flex items-center justify-center whitespace-nowrap rounded px-2 tabular-nums"
    :class="[config.bgColor, config.textColor]"
    role="status"
    :aria-label="accessibleLabel"
  >
    <span>{{ label }}</span>
    <span v-if="progress" class="inline-flex" aria-hidden="true">
      <span
        v-for="dot in 3"
        :key="dot"
        class="inline-block motion-safe:animate-judge-dot"
        :style="{ animationDelay: `${(dot - 1) * 0.2}s` }"
        >.</span
      >
    </span>
  </div>
</template>
