<script setup lang="ts">
import BorderedButton from '@/components/Controls/BorderedButton.vue'
defineProps<{ page: number; hasPrevious: boolean; hasNext: boolean; loading: boolean }>()
defineEmits<{ previous: []; next: [] }>()
</script>
<template>
  <nav class="mt-6 flex items-center justify-center gap-4" aria-label="ページ送り">
    <BorderedButton :disabled="loading || !hasPrevious" @click="$emit('previous')"
      >前へ</BorderedButton
    >
    <span>{{ page }}ページ</span>
    <BorderedButton :disabled="loading || !hasNext" @click="$emit('next')">次へ</BorderedButton>
  </nav>
</template>
