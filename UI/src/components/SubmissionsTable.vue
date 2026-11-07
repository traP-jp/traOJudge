<script setup lang="ts">
import { computed } from 'vue'
import { SubmissionsApi, ProblemsApi, type JudgeRun, type Submission } from '@/api/generated'
import { useCursorPagination } from '@/composables/useCursorPagination'
import CursorPagination from '@/components/Controls/Pagination/CursorPagination.vue'
import ListingTable, { type Column } from '@/components/ListingTable.vue'
import TextLink from '@/components/TextLink.vue'
import JudgeResultBadge from '@/components/JudgeResultBadge.vue'
import { dateToString } from '@/utils/date'
const props = defineProps<{ problemId?: string }>()
const api = new SubmissionsApi()
const problemsApi = new ProblemsApi()
type Row = { submission: Submission; title: string; run: JudgeRun | null }
const { items, page, hasNext, hasPrevious, isLoading, error, next, previous } =
  useCursorPagination<Row>(
    async (cursor, signal) => {
      let result = await api.listMySubmissions({ cursor, limit: 20 }, { signal })
      let matches = result.items.filter(
        (item) => !props.problemId || item.problemId === props.problemId
      )
      const seen = new Set<string>()
      while (!matches.length && result.nextCursor) {
        if (seen.has(result.nextCursor)) throw new Error('提出一覧を取得できませんでした．')
        seen.add(result.nextCursor)
        result = await api.listMySubmissions({ cursor: result.nextCursor, limit: 20 }, { signal })
        matches = result.items.filter(
          (item) => !props.problemId || item.problemId === props.problemId
        )
      }
      const rows = await Promise.all(
        matches.map(async (submission) => {
          const [problem, run] = await Promise.all([
            problemsApi
              .getProblem({ problemId: submission.problemId }, { signal })
              .catch(() => null),
            api
              .getJudgeRun(
                { submissionId: submission.id, judgeRunId: submission.currentJudgeRunId },
                { signal }
              )
              .catch(() => null)
          ])
          return { submission, title: problem?.title ?? submission.problemId, run }
        })
      )
      return { items: rows, nextCursor: result.nextCursor }
    },
    () => props.problemId
  )
const rows = computed(() => new Map(items.value.map((item) => [item.submission.id, item])))
const cols: (Column & { name: string })[] = [
  { id: 'submittedAt', textAlign: 'start', name: '提出日時', width: '176px' },
  { id: 'title', textAlign: 'start', name: '問題' },
  { id: 'score', textAlign: 'end', name: '得点', width: '64px' },
  { id: 'result', textAlign: 'center', name: '結果', width: '96px' },
  { id: 'time', textAlign: 'end', name: '実行時間', width: '112px' },
  { id: 'memory', textAlign: 'end', name: 'メモリ', width: '112px' }
]
</script>
<template>
  <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
  <p v-else-if="isLoading">読み込み中...</p>
  <p v-else-if="!items.length">提出はありません．</p>
  <ListingTable v-else :cols="cols" :row-ids="[...rows.keys()]">
    <template #head="{ colId }">{{ cols.find((col) => col.id === colId)?.name }}</template>
    <template #cell="{ rowId, colId }">
      <TextLink
        v-if="colId === 'submittedAt'"
        :href="`/problems/${rows.get(rowId)?.submission.problemId}/submissions/${rowId}`"
        >{{ dateToString(rows.get(rowId)?.submission.submittedAt) }}</TextLink
      >
      <TextLink
        v-else-if="colId === 'title'"
        :href="`/problems/${rows.get(rowId)?.submission.problemId}`"
        >{{ rows.get(rowId)?.title }}</TextLink
      >
      <template v-else-if="colId === 'score'">{{
        rows.get(rowId)?.run?.result?.score ?? '—'
      }}</template>
      <template v-else-if="colId === 'result'">
        <JudgeResultBadge
          v-if="rows.get(rowId)?.run?.result"
          :status="rows.get(rowId)!.run!.result!.verdict"
        />
        <span v-else>{{
          rows.get(rowId)?.run?.state === 'failed'
            ? '採点失敗'
            : rows.get(rowId)?.run
              ? '採点中'
              : '取得できません'
        }}</span>
      </template>
      <template v-else-if="colId === 'time'"
        >{{ rows.get(rowId)?.run?.result?.maxTimeMs ?? '—' }} ms</template
      >
      <template v-else
        >{{
          rows.get(rowId)?.run?.result
            ? Math.ceil(rows.get(rowId)!.run!.result!.maxMemoryBytes / 1024)
            : '—'
        }}
        KiB</template
      >
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
