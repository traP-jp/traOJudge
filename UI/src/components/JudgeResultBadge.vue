<script setup lang="ts">
import { computed } from 'vue'
import type { JudgeStatus } from '@/utils/statusConfig'
import { statusConfig } from '@/utils/statusConfig'

const props = defineProps<{
  status: JudgeStatus
}>()

const config = computed(() => statusConfig[props.status] || statusConfig.WJ)
</script>

<template>
  <div
    class="fontstyle-ui-control-strong inline-flex items-center justify-center rounded px-2"
    :class="[config.bgColor, config.textColor]"
  >
    {{ config.label }}
  </div>
</template>
