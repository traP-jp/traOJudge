<script setup lang="ts">
import { EditorialsApi } from '@/api/generated'
import { useCursorPagination } from '@/composables/useCursorPagination'
import CursorPagination from '@/components/Controls/Pagination/CursorPagination.vue'
import TextLink from '@/components/TextLink.vue'
const props = defineProps<{ problemId: string }>()
const api = new EditorialsApi()
const { items, page, hasNext, hasPrevious, isLoading, error, next, previous } = useCursorPagination(
  (cursor, signal) =>
    api.listProblemEditorials({ problemId: props.problemId, cursor, limit: 20 }, { signal }),
  () => props.problemId
)
</script>
<template>
  <div class="flex flex-col gap-4">
    <TextLink :href="`/problems/${problemId}/edit/editorials/new`">解説を追加</TextLink>
    <p class="text-text-secondary">解説の削除・公開状態の変更は準備中です．</p>
    <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
    <p v-else-if="isLoading">読み込み中...</p>
    <p v-else-if="!items.length">解説はまだありません．</p>
    <ul class="flex flex-col gap-3">
      <li v-for="editorial in items" :key="editorial.id">
        <TextLink :href="`/problems/${problemId}/edit/editorials/${editorial.id}`">{{
          editorial.title
        }}</TextLink>
        <span class="ml-4 text-text-secondary">{{
          editorial.visibility === 'public' ? '公開' : '非公開'
        }}</span>
      </li>
    </ul>
    <CursorPagination
      :page="page"
      :has-next="hasNext"
      :has-previous="hasPrevious"
      :loading="isLoading"
      @next="next"
      @previous="previous"
    />
  </div>
</template>
