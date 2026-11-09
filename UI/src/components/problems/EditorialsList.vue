<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import { EditorialsApi, type EditorialSummary } from '@/api/generated'
import MaterialIcon from '@/components/icons/MaterialIcon.vue'
import ListingTable from '@/components/ListingTable.vue'
import UserName from '@/components/users/UserName.vue'
import type { Column } from '@/types/listingTable'
import { dateToString } from '@/utils/date'

const { problemId } = defineProps<{
  problemId: string
}>()

const router = useRouter()

const isLoaded = ref<boolean>(false)
const editorialIds = ref<string[]>([])
const editorials = ref<Map<string, EditorialSummary>>(new Map())

const editorialsApi = new EditorialsApi()

async function loadEditorials() {
  isLoaded.value = false
  try {
    const summaries: EditorialSummary[] = await editorialsApi.getEditorialsOnProblem({
      problemId
    })

    editorialIds.value = summaries.map(({ id }) => id)
    editorials.value = new Map(summaries.map((editorial) => [editorial.id, editorial]))
  } catch (error) {
    console.error('API Error:', error)
    alert(`API Error: ${String(error)}`)
  } finally {
    isLoaded.value = true
  }
}

onMounted(loadEditorials)

const cols: (Column & { name: string })[] = [
  { id: 'title', textAlign: 'start', name: 'タイトル' },
  { id: 'owner', textAlign: 'start', name: '著者', width: '200px' },
  { id: 'updatedAt', textAlign: 'start', name: '最終更新', width: '150px' }
] as const

function handleEditorialClick(editorialId: string) {
  return router.push(`/problems/${problemId}/editorials/${editorialId}`)
}

function handleUserClick(userId: string) {
  return router.push(`/users/${userId}`)
}
</script>

<template>
  <ListingTable v-if="isLoaded" :cols="cols" :row-ids="editorialIds" :selected-rows="[]">
    <template #head="{ colId }">
      {{ cols.find(({ id }) => id === colId)?.name }}
    </template>
    <template #cell="{ rowId, colId }">
      <template v-if="colId === 'title'">
        <button
          class="flex items-center gap-1 text-text-primary hover:underline"
          @click="handleEditorialClick(rowId)"
        >
          {{ editorials.get(rowId)?.title }}
          <MaterialIcon icon="open_in_new" size="20px" />
        </button>
      </template>
      <template v-else-if="colId === 'owner'">
        <button
          class="flex items-center gap-1 text-text-primary hover:underline"
          @click="handleUserClick(editorials.get(rowId)?.authorId || '')"
        >
          <UserName :user-id="editorials.get(rowId)?.authorId || ''" />
          <MaterialIcon icon="open_in_new" size="20px" />
        </button>
      </template>
      <template v-else-if="colId === 'updatedAt'">
        {{ dateToString(editorials.get(rowId)?.updatedAt) }}
      </template>
      <template v-else>Unknown column: {{ colId }}</template>
    </template>
  </ListingTable>
  <div v-else>読み込み中...</div>
</template>
