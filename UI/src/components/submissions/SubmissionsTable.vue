<script setup lang="ts">
import { computed } from 'vue'

import type { SubmissionSummary } from '@/api/generated'
import JudgeResultBadge from '@/components/submissions/JudgeResultBadge.vue'
import ListingTable from '@/components/ListingTable.vue'
import TextLink from '@/components/TextLink.vue'
import type { Column } from '@/types/listingTable'
import { dateToString } from '@/utils/date'

export type SubmissionRow = Pick<
  SubmissionSummary,
  | 'id'
  | 'submittedAt'
  | 'title'
  | 'userName'
  | 'totalScore'
  | 'judgeStatus'
  | 'maxTime'
  | 'maxMemory'
> & { problemHref: string; userHref: string }
const props = defineProps<{
  rows: SubmissionRow[]
  columns: ('title' | 'userName')[]
}>()
const submissions = computed(() => new Map(props.rows.map((row) => [row.id, row])))
const cols: (Column & { name: string })[] = [
  { id: 'submittedAt', textAlign: 'start', name: '提出日時', width: '176px' },
  { id: 'title', textAlign: 'start', name: '問題' },
  { id: 'userName', textAlign: 'start', name: '提出者', width: '200px' },
  { id: 'totalScore', textAlign: 'end', name: '得点', width: '64px' },
  { id: 'judgeStatus', textAlign: 'center', name: '結果', width: '80px' },
  { id: 'maxTime', textAlign: 'end', name: '実行時間', width: '112px' },
  { id: 'maxMemory', textAlign: 'end', name: 'メモリ', width: '112px' }
] as const
</script>

<template>
  <ListingTable
    :cols="
      cols.filter(
        (col) => (col.id !== 'title' && col.id !== 'userName') || props.columns.includes(col.id)
      )
    "
    :row-ids="[...submissions.keys()]"
  >
    <template #head="{ colId }">
      {{ cols.find(({ id }) => id === colId)?.name }}
    </template>
    <template #cell="{ rowId, colId }">
      <template v-if="colId === 'submittedAt'">
        {{ dateToString(submissions.get(rowId)?.submittedAt) }}
      </template>
      <template v-else-if="colId === 'title'">
        <TextLink :href="submissions.get(rowId)?.problemHref ?? ''" :new-tab="true">
          {{ submissions.get(rowId)?.title }}
        </TextLink>
      </template>
      <template v-else-if="colId === 'userName'">
        <TextLink :href="submissions.get(rowId)?.userHref ?? ''" :new-tab="true">
          {{ submissions.get(rowId)?.userName }}
        </TextLink>
      </template>
      <template v-else-if="colId === 'totalScore'">
        {{ submissions.get(rowId)?.totalScore }}
      </template>
      <template v-else-if="colId === 'judgeStatus'">
        <JudgeResultBadge
          v-if="submissions.get(rowId)?.judgeStatus"
          :status="submissions.get(rowId)!.judgeStatus"
        />
      </template>
      <template v-else-if="colId === 'maxTime'">
        <span>{{ submissions.get(rowId)?.maxTime }}</span>
        <span class="ml-1 fontstyle-ui-body-2">ms</span>
      </template>
      <template v-else-if="colId === 'maxMemory'">
        <span>{{ submissions.get(rowId)?.maxMemory ?? 0 }}</span>
        <span class="ml-1 fontstyle-ui-body-2">KiB</span>
      </template>
      <template v-else>Unknown column: {{ colId }}</template>
    </template>
  </ListingTable>
  <p v-if="rows.length === 0" class="py-6 text-text-secondary">提出がありません。</p>
</template>
