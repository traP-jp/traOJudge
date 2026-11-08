<script setup lang="ts">
import { computed, onScopeDispose, watch } from 'vue'
import {
  Configuration,
  ContestsApi,
  SubmissionsApi,
  UsersApi,
  ResponseError,
  type JudgeRun,
  type Language,
  type Submission
} from '@/api/generated'
import { useCursorPagination } from '@/composables/useCursorPagination'
import CursorPagination from '@/components/Controls/Pagination/CursorPagination.vue'
import BorderedButton from '@/components/Controls/BorderedButton.vue'
import ListingTable, { type Column } from '@/components/ListingTable.vue'
import TextLink from '@/components/TextLink.vue'
import JudgeRunBadge from '@/components/JudgeRunBadge.vue'
import { dateToString } from '@/utils/date'
import { isJudgeRunFinished, latestJudgeRun } from '@/utils/judgeRun'
import type {
  ContestSubmissionProblem,
  ContestSubmissionScope,
  ContestSubmissionTeam
} from '@/types/contestSubmissions'

const props = defineProps<{
  contestId: string
  scope: ContestSubmissionScope
  viewerId?: string
  teamId: string
  problemId: string
  problems: ContestSubmissionProblem[]
  teams: ContestSubmissionTeam[]
  languages: Language[]
  canViewAll: boolean
  teamContest: boolean
}>()
const config = new Configuration({
  basePath:
    import.meta.env.VITE_CONTEST_API_BASE_URL || import.meta.env.VITE_API_BASE_URL || '/api/v1',
  credentials: 'include'
})
const contestsApi = new ContestsApi(config)
const submissionsApi = new SubmissionsApi(config)
const usersApi = new UsersApi(config)
type Row = {
  submission: Submission
  userName: string
  run: JudgeRun | null
  error: string
  retry: boolean
}
const userNames = new Map<string, string>()

async function loadUserName(userId: string, signal: AbortSignal): Promise<string> {
  const cached = userNames.get(userId)
  if (cached) return cached
  const user = await usersApi.getUserProfile({ userId }, { signal }).catch(() => null)
  if (!user || signal.aborted) return userId
  userNames.set(userId, user.displayName)
  return user.displayName
}

function matches(submission: Submission): boolean {
  return (
    submission.context.kind === 'contest' &&
    submission.context.contestId === props.contestId &&
    (!props.problemId || submission.problemId === props.problemId) &&
    (props.scope !== 'mine' || submission.userId === props.viewerId) &&
    (props.scope !== 'team' || submission.teamId === props.teamId)
  )
}

async function loadSubmissions(cursor: string | undefined, signal: AbortSignal) {
  if (props.scope === 'mine')
    return submissionsApi.listMySubmissions({ cursor, limit: 20 }, { signal })
  if (props.scope === 'team' && !props.canViewAll)
    return contestsApi.listContestTeamSubmissions(
      { contestId: props.contestId, teamId: props.teamId, cursor, limit: 20 },
      { signal }
    )
  return contestsApi.listContestSubmissions(
    { contestId: props.contestId, cursor, limit: 20 },
    { signal }
  )
}

async function loadRun(row: Row, signal: AbortSignal, refresh: boolean): Promise<Row> {
  try {
    const submission = refresh
      ? await submissionsApi.getSubmission({ submissionId: row.submission.id }, { signal })
      : row.submission
    const run = await submissionsApi.getJudgeRun(
      { submissionId: submission.id, judgeRunId: submission.currentJudgeRunId },
      { signal }
    )
    return { ...row, submission, run: latestJudgeRun(row.run, run), error: '', retry: true }
  } catch (cause) {
    const status = cause instanceof ResponseError ? cause.response.status : undefined
    const retry = ![401, 403, 404].includes(status ?? 0)
    return {
      ...row,
      retry,
      error: retry ? '判定を取得できません．再取得しています．' : '判定を閲覧できません．'
    }
  }
}

const { items, page, hasNext, hasPrevious, isLoading, error, next, previous, reload } =
  useCursorPagination<Row>(
    async (cursor, signal) => {
      let result = await loadSubmissions(cursor, signal)
      let submissions = result.items.filter(matches)
      const seen = new Set(cursor ? [cursor] : [])
      // APIには問題・ユーザーの絞り込みがないため，一致する提出があるページまで読み進める．
      while (!submissions.length && result.nextCursor) {
        signal.throwIfAborted()
        if (seen.has(result.nextCursor)) throw new Error('提出一覧の続きを取得できませんでした．')
        seen.add(result.nextCursor)
        result = await loadSubmissions(result.nextCursor, signal)
        submissions = result.items.filter(matches)
      }
      const names = new Map(
        await Promise.all(
          [...new Set(submissions.map((item) => item.userId))].map(
            async (id) => [id, await loadUserName(id, signal)] as const
          )
        )
      )
      const rows = await Promise.all(
        submissions.map((submission) =>
          loadRun(
            {
              submission,
              userName: names.get(submission.userId) ?? submission.userId,
              run: null,
              error: '',
              retry: true
            },
            signal,
            false
          )
        )
      )
      return { items: rows, nextCursor: result.nextCursor }
    },
    () => [
      props.contestId,
      props.scope,
      props.viewerId,
      props.teamId,
      props.problemId,
      props.canViewAll
    ]
  )

let timer: ReturnType<typeof setTimeout> | undefined
let controller: AbortController | undefined
function pending(row: Row): boolean {
  return row.retry && !isJudgeRunFinished(row.run)
}
function schedule(signal: AbortSignal): void {
  if (signal.aborted || !items.value.some(pending)) return
  timer = setTimeout(() => void refreshPending(signal), 2000)
}
async function refreshPending(signal: AbortSignal): Promise<void> {
  const updated = await Promise.all(
    items.value.map((row) => (pending(row) ? loadRun(row, signal, true) : row))
  )
  if (signal.aborted) return
  items.value = updated
  schedule(signal)
}
watch(
  isLoading,
  (loading) => {
    clearTimeout(timer)
    controller?.abort()
    if (loading || error.value) return
    controller = new AbortController()
    schedule(controller.signal)
  },
  { immediate: true, flush: 'sync' }
)
onScopeDispose(() => {
  clearTimeout(timer)
  controller?.abort()
})

const rows = computed(() => new Map(items.value.map((row) => [row.submission.id, row])))
const problemMap = computed(
  () => new Map(props.problems.map((problem) => [problem.problemId, problem]))
)
const teamNames = computed(() => new Map(props.teams.map((team) => [team.id, team.name])))
const languageNames = computed(
  () => new Map(props.languages.map((language) => [language.id, language.name]))
)
const pollErrors = computed(() => [...new Set(items.value.map((row) => row.error).filter(Boolean))])
const cols = computed<(Column & { name: string })[]>(() => [
  { id: 'submittedAt', textAlign: 'start', name: '提出日時' },
  { id: 'problem', textAlign: 'start', name: '問題' },
  { id: 'user', textAlign: 'start', name: '提出者' },
  ...(props.teamContest ? [{ id: 'team', textAlign: 'start' as const, name: 'チーム' }] : []),
  { id: 'language', textAlign: 'start', name: '言語' },
  { id: 'score', textAlign: 'end', name: '得点' },
  { id: 'result', textAlign: 'center', name: '結果', width: '144px' },
  { id: 'time', textAlign: 'end', name: '実行時間' },
  { id: 'memory', textAlign: 'end', name: 'メモリ' }
])
</script>

<template>
  <div class="space-y-4" :aria-busy="isLoading">
    <div class="flex justify-end">
      <BorderedButton :disabled="isLoading" icon="refresh" @click="reload">更新</BorderedButton>
    </div>
    <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
    <p v-else-if="isLoading" role="status" class="py-8 text-center">提出を読み込み中…</p>
    <p v-else-if="!items.length" role="status" class="py-8 text-center text-text-secondary">
      条件に一致する提出はありません．
    </p>
    <div v-else class="overflow-x-auto">
      <ListingTable
        :cols="cols"
        :row-ids="[...rows.keys()]"
        row-header-column="submittedAt"
        caption="コンテストの提出一覧"
        class="whitespace-nowrap"
      >
        <template #head="{ colId }">{{ cols.find((col) => col.id === colId)?.name }}</template>
        <template #cell="{ rowId, colId }">
          <time
            v-if="colId === 'submittedAt'"
            :datetime="rows.get(rowId)?.submission.submittedAt.toISOString()"
            >{{ dateToString(rows.get(rowId)?.submission.submittedAt) }}</time
          >
          <template v-else-if="colId === 'problem'">
            <TextLink
              v-if="problemMap.get(rows.get(rowId)!.submission.problemId)?.canOpen"
              :href="`/problems/${rows.get(rowId)!.submission.problemId}`"
              >{{ problemMap.get(rows.get(rowId)!.submission.problemId)?.label }} —
              {{ problemMap.get(rows.get(rowId)!.submission.problemId)?.title }}</TextLink
            >
            <span v-else>{{
              problemMap.get(rows.get(rowId)!.submission.problemId)?.title ??
              rows.get(rowId)?.submission.problemId
            }}</span>
          </template>
          <template v-else-if="colId === 'user'">{{ rows.get(rowId)?.userName }}</template>
          <template v-else-if="colId === 'team'">{{
            teamNames.get(rows.get(rowId)?.submission.teamId ?? '') ??
            rows.get(rowId)?.submission.teamId ??
            '—'
          }}</template>
          <template v-else-if="colId === 'language'">{{
            languageNames.get(rows.get(rowId)!.submission.languageId) ??
            rows.get(rowId)?.submission.languageId
          }}</template>
          <template v-else-if="colId === 'score'">{{
            rows.get(rowId)?.run?.result?.score ?? '—'
          }}</template>
          <template v-else-if="colId === 'result'">
            <JudgeRunBadge v-if="rows.get(rowId)?.run" :run="rows.get(rowId)!.run!" />
            <span v-else class="text-text-secondary">{{
              rows.get(rowId)?.retry ? '取得失敗' : '閲覧不可'
            }}</span>
          </template>
          <template v-else-if="colId === 'time'">{{
            rows.get(rowId)?.run?.result ? `${rows.get(rowId)!.run!.result!.maxTimeMs} ms` : '—'
          }}</template>
          <template v-else-if="colId === 'memory'">{{
            rows.get(rowId)?.run?.result
              ? `${Math.ceil(rows.get(rowId)!.run!.result!.maxMemoryBytes / 1024)} KiB`
              : '—'
          }}</template>
        </template>
      </ListingTable>
    </div>
    <p
      v-for="message in pollErrors"
      v-show="!isLoading && !error"
      :key="message"
      role="status"
      class="text-status-error"
    >
      {{ message }}
    </p>
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
