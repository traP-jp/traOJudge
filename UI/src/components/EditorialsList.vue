<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { EditorialsApi, UsersApi, type Profile } from '@/api/generated'
import { useCursorPagination } from '@/composables/useCursorPagination'
import ListingTable, { type Column } from '@/components/ListingTable.vue'
import CursorPagination from '@/components/Controls/Pagination/CursorPagination.vue'
import TextLink from '@/components/TextLink.vue'
import { dateToString } from '@/utils/date'
const props = defineProps<{ problemId: string }>()
const api = new EditorialsApi()
const usersApi = new UsersApi()
const users = ref(new Map<string, Profile>())
const { items, page, hasNext, hasPrevious, isLoading, error, next, previous } = useCursorPagination(
  (cursor, signal) =>
    api.listProblemEditorials({ problemId: props.problemId, cursor, limit: 20 }, { signal }),
  () => props.problemId
)
const editorials = computed(() => new Map(items.value.map((item) => [item.id, item])))
watch(items, async (value) => {
  const ids = [...new Set(value.map((item) => item.authorId))]
  const profiles = await Promise.all(
    ids.map((userId) => usersApi.getUserProfile({ userId }).catch(() => null))
  )
  users.value = new Map(
    profiles
      .filter((profile): profile is Profile => profile !== null)
      .map((profile) => [profile.id, profile])
  )
})
const cols: (Column & { name: string })[] = [
  { id: 'title', textAlign: 'start', name: 'タイトル' },
  { id: 'author', textAlign: 'start', name: '著者', width: '200px' },
  { id: 'updatedAt', textAlign: 'start', name: '最終更新', width: '176px' }
]
</script>
<template>
  <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
  <p v-else-if="isLoading">読み込み中...</p>
  <p v-else-if="!items.length">解説はまだありません．</p>
  <ListingTable v-else :cols="cols" :row-ids="[...editorials.keys()]">
    <template #head="{ colId }">{{ cols.find((col) => col.id === colId)?.name }}</template>
    <template #cell="{ rowId, colId }">
      <TextLink v-if="colId === 'title'" :href="`/problems/${problemId}/editorials/${rowId}`">{{
        editorials.get(rowId)?.title
      }}</TextLink>
      <TextLink
        v-else-if="colId === 'author'"
        :href="`/users/${editorials.get(rowId)?.authorId}`"
        >{{
          users.get(editorials.get(rowId)?.authorId ?? '')?.displayName ??
          editorials.get(rowId)?.authorId
        }}</TextLink
      >
      <template v-else>{{ dateToString(editorials.get(rowId)?.updatedAt) }}</template>
    </template>
  </ListingTable>
  <CursorPagination
    :page="page"
    :has-next="hasNext"
    :has-previous="hasPrevious"
    :loading="isLoading"
    @next="next"
    @previous="previous"
  />
</template>
