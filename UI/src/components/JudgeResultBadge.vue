<script setup lang="ts">
import type { JudgeStatus } from '@/api/generated'
import { statusConfig } from '@/utils/statusConfig'

const props = defineProps<{
  status: JudgeStatus
}>()

const config = statusConfig[props.status] || statusConfig.WJ
</script>

<template>
  <div
    class="inline-flex items-center justify-center rounded-sm px-2 fontstyle-ui-control-strong"
    :class="[config.bgColor, config.textColor]"
  >
    {{ config.label }}
  </div>
</template>
