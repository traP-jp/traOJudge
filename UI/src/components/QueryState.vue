<script setup lang="ts" generic="T">
import MonochromeButton from '@/components/Controls/MonochromeButton.vue'
import type { QueryView } from '@/composables/useSessionQuery'

defineProps<{ view: QueryView<T> }>()
defineEmits<{ retry: [] }>()
</script>

<template>
  <div v-if="view.status === 'pending'" role="status" class="py-8 text-text-secondary">
    読み込み中...
  </div>
  <div
    v-else-if="view.status === 'error'"
    role="alert"
    class="flex flex-col items-start gap-3 py-4"
  >
    <p class="text-status-error">{{ view.message }}</p>
    <MonochromeButton small @click="$emit('retry')">再試行</MonochromeButton>
  </div>
  <template v-else>
    <div v-if="view.warning" role="alert" class="flex items-center gap-3 text-status-error">
      <span>{{ view.warning }}</span>
      <MonochromeButton small @click="$emit('retry')">再試行</MonochromeButton>
    </div>
    <span v-else-if="view.refreshing" role="status" class="text-text-secondary">更新中...</span>
    <slot :data="view.data" />
  </template>
</template>
