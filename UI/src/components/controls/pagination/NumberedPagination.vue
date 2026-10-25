<script setup lang="ts">
import { watch } from 'vue'

import SimplePagination from '@/components/controls/pagination/SimplePagination.vue'

const props = defineProps<{ page: number; totalPages: number }>()
const emit = defineEmits<{ change: [page: number]; normalize: [page: number] }>()
watch(
  () => [props.page, props.totalPages],
  () => {
    const bounded = Math.max(1, Math.min(props.page, props.totalPages))
    if (bounded !== props.page) emit('normalize', bounded)
  },
  { immediate: true }
)
</script>

<template>
  <SimplePagination
    v-if="totalPages > 1"
    :model-value="page"
    :end="totalPages"
    class="mt-6"
    @update:model-value="emit('change', $event)"
  />
</template>
