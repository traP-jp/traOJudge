<script setup lang="ts">
import { computed } from 'vue'
import type { ContestProblem as ContestProblem } from '@/api/generated/models/ContestProblem'
import ListingTable, { type Column } from '@/components/ListingTable.vue'
import type {
  FirstAccept,
  ProblemResult,
  ProblemStat,
  StandingRow,
  StandingsKind
} from '@/types/standings'
import {
  findStandingRowForUser,
  formatAcceptedElapsedTime,
  getLastAcceptedAt,
  type StandingSelection,
  type StandingsSort
} from '@/utils/standings'

const props = defineProps<{
  kind: StandingsKind
  problems: ContestProblem[]
  problemStats: Record<string, ProblemStat>
  rows: StandingRow[]
  allRows: StandingRow[]
  sort: StandingsSort
  startsAt: Date
  firstAccepts?: Record<string, FirstAccept>
  highlightedRowId?: string
  highlightedProblemId?: string
}>()
const emit = defineEmits<{
  (event: 'sort', column: string): void
  (event: 'selectFirstSolver', selection: StandingSelection): void
}>()

const caption = computed(() => {
  if (props.kind === 'team') return 'チーム順位表'
  if (props.kind === 'extended') return '延長戦順位表'
  return '個人順位表'
})
const rowById = computed(() => new Map(props.rows.map((row) => [row.id, row])))
const columns = computed<Column[]>(() => [
  { id: 'rank', textAlign: 'end', width: '80px' },
  { id: 'name', textAlign: 'start', width: '240px' },
  ...props.problems.map((problem) => ({
    id: 'problem:' + problem.problemId,
    textAlign: 'end' as const,
    width: '120px',
    ariaSort: ariaSort(problem.problemId)
  })),
  { id: 'total', textAlign: 'end', width: '128px', ariaSort: ariaSort('total') }
])

function ariaSort(column: string): 'ascending' | 'descending' | undefined {
  if (props.sort.column !== column) return undefined
  return props.sort.direction === 'desc' ? 'descending' : 'ascending'
}

function problemId(columnId: string): string {
  return columnId.slice('problem:'.length)
}

function problemLabel(columnId: string): string {
  return props.problems.find((problem) => problem.problemId === problemId(columnId))?.label ?? ''
}

function sortLabel(column: string): string {
  if (props.sort.column !== column) return '並べ替え'
  return props.sort.direction === 'desc' ? '降順' : '昇順'
}

function problemSortLabel(columnId: string): string {
  const id = problemId(columnId)
  if (!hasProblemScore(id)) return problemLabel(columnId) + 'の問題別得点は未提供'
  return problemLabel(columnId) + 'の得点で' + sortLabel(id)
}

function problemStatLabel(columnId: string): string {
  const stat = props.problemStats[problemId(columnId)]
  const unit = props.kind === 'team' ? '組' : '人'
  return '正解' + stat.solvedCount + unit + '，提出' + stat.submittedCount + unit
}

function hasProblemScore(id: string): boolean {
  return props.allRows.some((row) => row.results[id]?.score != null)
}

function resultFor(rowId: string, columnId: string): ProblemResult | undefined {
  return rowById.value.get(rowId)?.results[problemId(columnId)]
}

function acceptedTimeFor(rowId: string, columnId: string): string | null {
  return formatAcceptedElapsedTime(resultFor(rowId, columnId)?.acceptedAt, props.startsAt)
}

function lastAcceptedTimeFor(rowId: string): string | null {
  const row = rowById.value.get(rowId)
  return row ? formatAcceptedElapsedTime(getLastAcceptedAt(row.results), props.startsAt) : null
}

function firstAcceptFor(columnId: string): FirstAccept | undefined {
  return props.firstAccepts?.[problemId(columnId)]
}

function firstAcceptTimeFor(columnId: string): string | null {
  return formatAcceptedElapsedTime(firstAcceptFor(columnId)?.acceptedAt, props.startsAt)
}

function firstAcceptHasStanding(columnId: string): boolean {
  const userId = firstAcceptFor(columnId)?.id
  return userId !== undefined && findStandingRowForUser(props.allRows, userId) !== undefined
}

function selectFirstSolver(columnId: string): void {
  const userId = firstAcceptFor(columnId)?.id
  if (userId) emit('selectFirstSolver', { userId, problemId: problemId(columnId) })
}
</script>

<template>
  <div class="overflow-x-auto rounded border border-border-primary">
    <ListingTable
      :cols="columns"
      :row-ids="rows.map((row) => row.id)"
      :caption="caption"
      row-header-column="name"
      :show-footer="firstAccepts !== undefined"
      :footer-label-colspan="2"
      :highlighted-row-id="highlightedRowId"
      :highlighted-cell="
        highlightedRowId && highlightedProblemId
          ? { rowId: highlightedRowId, colId: 'problem:' + highlightedProblemId }
          : undefined
      "
      class="min-w-full whitespace-nowrap text-sm"
    >
      <template #head="{ colId }">
        <span v-if="colId === 'rank'">順位</span>
        <span v-else-if="colId === 'name'">{{ kind === 'team' ? 'チーム' : 'ユーザー' }}</span>
        <template v-else-if="colId.startsWith('problem:')">
          <button
            type="button"
            :disabled="!hasProblemScore(problemId(colId))"
            class="inline-flex min-h-8 items-center gap-1 rounded px-1 focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand-primary enabled:hover:text-brand-primary disabled:cursor-not-allowed disabled:text-text-tertiary"
            :aria-label="problemSortLabel(colId)"
            @click="emit('sort', problemId(colId))"
          >
            {{ problemLabel(colId) }}
            <span aria-hidden="true" class="text-xs">
              {{ sort.column === problemId(colId) ? (sort.direction === 'desc' ? '▼' : '▲') : '↕' }}
            </span>
          </button>
          <span
            v-if="problemStats[problemId(colId)]"
            class="block text-xs font-normal text-text-secondary"
            :aria-label="problemStatLabel(colId)"
          >
            {{ problemStats[problemId(colId)]?.solvedCount }}/{{
              problemStats[problemId(colId)]?.submittedCount
            }}{{ kind === 'team' ? '組' : '人' }}
          </span>
        </template>
        <button
          v-else-if="colId === 'total'"
          type="button"
          class="inline-flex min-h-8 items-center gap-1 rounded px-1 hover:text-brand-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand-primary"
          :aria-label="'合計得点で' + sortLabel('total')"
          @click="emit('sort', 'total')"
        >
          合計得点
          <span aria-hidden="true" class="text-xs">
            {{ sort.column === 'total' ? (sort.direction === 'desc' ? '▼' : '▲') : '↕' }}
          </span>
        </button>
      </template>
      <template #cell="{ rowId, colId }">
        <span v-if="colId === 'rank'" class="tabular-nums">{{ rowById.get(rowId)?.rank }}</span>
        <template v-else-if="colId === 'name'">
          <RouterLink
            v-if="kind !== 'team'"
            :to="'/users/' + encodeURIComponent(rowId)"
            class="text-brand-primary hover:underline"
          >
            {{ rowById.get(rowId)?.name }}
          </RouterLink>
          <span v-else>{{ rowById.get(rowId)?.name }}</span>
          <span
            v-if="kind === 'team' && rowById.get(rowId)?.members?.length"
            class="mt-1 block text-xs font-normal text-text-secondary"
          >
            {{ rowById.get(rowId)?.members?.join('，') }}
          </span>
        </template>
        <template v-else-if="colId.startsWith('problem:')">
          <span v-if="resultFor(rowId, colId)?.score != null" class="tabular-nums">
            {{ resultFor(rowId, colId)?.score }}
          </span>
          <span v-else class="text-text-tertiary">—</span>
          <time
            v-if="acceptedTimeFor(rowId, colId)"
            :datetime="resultFor(rowId, colId)?.acceptedAt"
            class="block text-xs tabular-nums text-text-secondary"
          >
            {{ acceptedTimeFor(rowId, colId) }}
          </time>
          <span
            v-if="resultFor(rowId, colId)?.pendingCount"
            class="ml-1 text-xs text-status-pending"
            :aria-label="'反映待ち' + resultFor(rowId, colId)?.pendingCount + '件'"
          >
            +{{ resultFor(rowId, colId)?.pendingCount }}待ち
          </span>
        </template>
        <template v-else-if="colId === 'total'">
          <span class="font-semibold tabular-nums">{{ rowById.get(rowId)?.score }}</span>
          <time
            v-if="lastAcceptedTimeFor(rowId)"
            :datetime="getLastAcceptedAt(rowById.get(rowId)?.results ?? {})"
            class="block text-xs tabular-nums text-text-secondary"
          >
            {{ lastAcceptedTimeFor(rowId) }}
          </time>
        </template>
      </template>
      <template #foot="{ colId }">
        <span v-if="colId === 'rank'" class="text-text-secondary">First Solver</span>
        <template v-else-if="colId.startsWith('problem:')">
          <template v-if="firstAcceptFor(colId)">
            <button
              v-if="firstAcceptHasStanding(colId)"
              type="button"
              class="text-brand-primary hover:underline"
              :aria-label="
                (firstAcceptFor(colId)?.name ?? '') + 'の' + problemLabel(colId) + 'の得点へ移動'
              "
              @click="selectFirstSolver(colId)"
            >
              {{ firstAcceptFor(colId)?.name }}
            </button>
            <RouterLink
              v-else
              :to="'/users/' + encodeURIComponent(firstAcceptFor(colId)?.id ?? '')"
              class="text-brand-primary hover:underline"
            >
              {{ firstAcceptFor(colId)?.name }}
            </RouterLink>
            <time
              v-if="firstAcceptTimeFor(colId)"
              :datetime="firstAcceptFor(colId)?.acceptedAt"
              class="block text-xs tabular-nums text-text-secondary"
            >
              {{ firstAcceptTimeFor(colId) }}
            </time>
          </template>
          <span v-else class="text-text-tertiary">—</span>
        </template>
      </template>
    </ListingTable>
  </div>
</template>
