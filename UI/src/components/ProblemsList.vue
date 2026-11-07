<script setup lang="ts">
import { computed, ref } from 'vue'
import { ProblemsApi } from '@/api/generated'
import { useCursorPagination } from '@/composables/useCursorPagination'
import CursorPagination from '@/components/Controls/Pagination/CursorPagination.vue'
import PlainTextbox from '@/components/Controls/Textbox/PlainTextbox.vue'
import ListingTable, { type Column } from '@/components/ListingTable.vue'
import TextLink from '@/components/TextLink.vue'
import { dateToString } from '@/utils/date'
const props = defineProps<{ createdByUserId?: string }>()
const query = ref('')
const api = new ProblemsApi()
const { items, page, hasPrevious, hasNext, isLoading, error, next, previous } = useCursorPagination(
  (cursor, signal) =>
    api.listProblems(
      {
        cursor,
        limit: 20,
        query: query.value.trim() || undefined,
        createdByUserId: props.createdByUserId
      },
      { signal }
    ),
  () => [query.value, props.createdByUserId]
)
const problems = computed(() => new Map(items.value.map((item) => [item.id, item])))
const cols: (Column & { name: string })[] = [
  { id: 'createdAt', textAlign: 'start', name: '投稿日時', width: '176px' },
  { id: 'title', textAlign: 'start', name: '問題' },
  { id: 'visibility', textAlign: 'start', name: '公開状態', width: '112px' }
]
</script>
<template>
  <div class="fontstyle-ui-subtitle flex items-center text-text-primary"><slot /></div>
  <PlainTextbox v-model="query" label="問題名で検索" placeholder="キーワードを入力" />
  <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
  <p v-else-if="isLoading">読み込み中...</p>
  <p v-else-if="!items.length">問題がありません．</p>
  <ListingTable v-else :cols="cols" :row-ids="[...problems.keys()]">
    <template #head="{ colId }">{{ cols.find((col) => col.id === colId)?.name }}</template>
    <template #cell="{ rowId, colId }">
      <template v-if="colId === 'createdAt'">{{
        dateToString(problems.get(rowId)?.createdAt)
      }}</template>
      <TextLink v-else-if="colId === 'title'" :href="`/problems/${rowId}`">{{
        problems.get(rowId)?.title
      }}</TextLink>
      <template v-else>{{
        problems.get(rowId)?.visibility === 'public' ? '公開' : '非公開'
      }}</template>
    </template>
  </ListingTable>
  <CursorPagination
    :page="page"
    :has-previous="hasPrevious"
    :has-next="hasNext"
    :loading="isLoading"
    @previous="previous"
    @next="next"
  />
</template>
