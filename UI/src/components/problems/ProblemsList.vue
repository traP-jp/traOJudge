<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import type { ProblemSummary } from '@/api/generated'
import DifficultyStar from '@/components/problems/DifficultyStar.vue'
import ListingTable from '@/components/ListingTable.vue'
import TextLink from '@/components/TextLink.vue'
import { problemLocation } from '@/router/locations'
import type { Column } from '@/types/listingTable'
import { dateToString } from '@/utils/date'

const props = defineProps<{ rows: ProblemSummary[] }>()
const router = useRouter()
const problems = computed(() => new Map(props.rows.map((problem) => [problem.id, problem])))
const cols: (Column & { name: string })[] = [
  { id: 'createdAt', textAlign: 'start', name: '投稿日時', width: '176px' },
  { id: 'title', textAlign: 'start', name: '問題' },
  { id: 'difficulty', textAlign: 'start', name: '難易度', width: '160px' },
  { id: 'solvedCount', textAlign: 'end', name: 'Solves', width: '112px' }
] as const
</script>

<template>
  <!-- TODO: add sorting features -->
  <!-- Nullish になり得ない所でも型安全性のため Non-null Assertion はしない -->
  <ListingTable :cols="cols" :row-ids="[...problems.keys()]">
    <template #head="{ colId }">
      {{ cols.find(({ id }) => id === colId)?.name }}
    </template>
    <template #cell="{ rowId, colId }">
      <!-- 文字列のみとは限らずリンクやアイコンなどを含めるようにするため、関数に切り出してはいない -->
      <template v-if="colId === 'createdAt'">
        {{ dateToString(problems.get(rowId)?.createdAt) }}
      </template>
      <template v-else-if="colId === 'title'">
        <TextLink :href="router.resolve(problemLocation(rowId)).href" new-tab>
          {{ problems.get(rowId)?.title }}
        </TextLink>
      </template>
      <template v-else-if="colId === 'difficulty'">
        <div class="flex">
          <DifficultyStar class="grow" :difficulty="problems.get(rowId)?.difficulty ?? 1" />
          <div>{{ problems.get(rowId)?.difficulty }}</div>
        </div>
      </template>
      <template v-else-if="colId === 'solvedCount'">
        {{ problems.get(rowId)?.solvedCount }}
      </template>
      <template v-else>Unknown column: {{ colId }}</template>
    </template>
  </ListingTable>
  <p v-if="rows.length === 0" class="py-6 text-text-secondary">問題がありません。</p>
</template>
