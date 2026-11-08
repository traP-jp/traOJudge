<script setup lang="ts">
import { computed } from 'vue'

import MonochromeButton from '@/components/controls/MonochromeButton.vue'

const {
  cursor,
  previousCursor,
  nextCursor,
  loading = false
} = defineProps<{
  cursor?: string
  previousCursor?: string | null
  nextCursor: string | null
  loading?: boolean
}>()
defineEmits<{ change: [cursor: string | undefined] }>()
const hasNext = computed(() => !!nextCursor && nextCursor !== cursor)
</script>

<template>
  <nav
    v-if="cursor || hasNext"
    aria-label="一覧のページ送り"
    class="flex justify-center gap-3 py-4"
  >
    <MonochromeButton
      small
      :disabled="loading || !cursor"
      @click="$emit('change', previousCursor ?? undefined)"
      >{{ cursor && previousCursor === undefined ? '最初に戻る' : '前へ' }}</MonochromeButton
    >
    <MonochromeButton
      small
      :disabled="loading || !hasNext"
      @click="$emit('change', nextCursor ?? undefined)"
      >次へ</MonochromeButton
    >
  </nav>
</template>
