<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { ContestsApi } from '@/api/generated/apis/ContestsApi'
import { MeApi } from '@/api/generated/apis/MeApi'
import type { Contest } from '@/api/generated/models/Contest'
import type { ContestProblem as ContestProblem } from '@/api/generated/models/ContestProblem'
import { Configuration, ResponseError } from '@/api/generated/runtime'
import {
  getExtendedStandingBoard,
  getIndividualStandingDetails,
  getTeamStandingDetails,
  StandingsPreviewError
} from '@/api/standingsPreview'
import BorderedButton from '@/components/Controls/BorderedButton.vue'
import SimplePagination from '@/components/Controls/Pagination/SimplePagination.vue'
import PlainTextbox from '@/components/Controls/Textbox/PlainTextbox.vue'
import ContestStandingsTable from '@/components/ContestStandingsTable.vue'
import ContestHeader from '@/components/Navigations/ContestHeader.vue'
import { useUserStore } from '@/stores/user'
import type {
  IndividualStandingDetails,
  StandingBoard,
  StandingsKind,
  TeamStandingDetails
} from '@/types/standings'
import { formatContestDate, getContestPhase } from '@/utils/contest'
import { canViewContestStandings, loadContestViewerRoles } from '@/utils/contestStandingsAccess'
import {
  findStandingRowForUser,
  mergeStandingProblemScores,
  sortStandingRows,
  type StandingSelection,
  type StandingsSort
} from '@/utils/standings'

const props = defineProps<{ kind: StandingsKind }>()
const route = useRoute()
const userStore = useUserStore()
const contest = ref<Contest | null>(null)
const problems = ref<ContestProblem[]>([])
const board = ref<StandingBoard | null>(null)
const isLoading = ref(true)
const isRefreshing = ref(false)
const error = ref('')
const refreshError = ref('')
const notice = ref('')
const sort = ref<StandingsSort>({ column: 'total', direction: 'desc' })
const search = ref('')
const page = ref(1)
const selectedStanding = ref<StandingSelection | null>(null)
const pageSize = 10
const config = new Configuration({
  basePath:
    import.meta.env.VITE_CONTEST_API_BASE_URL || import.meta.env.VITE_API_BASE_URL || '/api/v1',
  credentials: 'include'
})
const contestsApi = new ContestsApi(config)
const meApi = new MeApi(config)
let requestController: AbortController | undefined
let transitionTimer: ReturnType<typeof setTimeout> | undefined

const labels: Record<StandingsKind, string> = {
  individual: '個人順位表',
  team: 'チーム順位表',
  extended: '延長戦順位表'
}
const title = computed(() => labels[props.kind])
const standingsTabs = computed(() => [
  contest.value?.participationMode === 'team'
    ? { name: 'contest-standings-team', label: 'チーム' }
    : { name: 'contest-standings', label: '個人' },
  { name: 'contest-standings-extended', label: '延長戦' }
])
const searchLabel = computed(() =>
  props.kind === 'team' ? 'チーム・メンバーを検索' : 'ユーザーを検索'
)
const sortedRows = computed(() => {
  const rows = board.value?.items ?? []
  const query = search.value.trim().toLocaleLowerCase('ja')
  const filtered = query
    ? rows.filter((row) =>
        [row.id, row.name, ...(row.members ?? [])].some((value) =>
          value.toLocaleLowerCase('ja').includes(query)
        )
      )
    : rows
  return sortStandingRows(filtered, sort.value)
})
const totalPages = computed(() => Math.ceil(sortedRows.value.length / pageSize))
const pageRows = computed(() =>
  sortedRows.value.slice((page.value - 1) * pageSize, page.value * pageSize)
)
const highlightedRowId = computed(() => {
  const target = selectedStanding.value
  return target && board.value
    ? findStandingRowForUser(board.value.items, target.userId)?.id
    : undefined
})
const highlightedProblemId = computed(() => {
  const problemId = selectedStanding.value?.problemId
  return problemId && problems.value.some((problem) => problem.problemId === problemId)
    ? problemId
    : undefined
})
function changeSort(column: string): void {
  page.value = 1
  sort.value = {
    column,
    direction: sort.value.column === column && sort.value.direction === 'desc' ? 'asc' : 'desc'
  }
}

async function selectFirstSolver(selection: StandingSelection): Promise<void> {
  if (!board.value || !findStandingRowForUser(board.value.items, selection.userId)) return
  if (!problems.value.some((problem) => problem.problemId === selection.problemId)) return
  selectedStanding.value = selection
  search.value = ''
  await nextTick()
  const index = sortedRows.value.findIndex((row) => row.id === highlightedRowId.value)
  if (index < 0) return
  page.value = Math.floor(index / pageSize) + 1
  await nextTick()
  const cell = document.getElementById('highlighted-table-cell')
  cell?.scrollIntoView({ block: 'center', inline: 'nearest' })
  cell?.focus({ preventScroll: true })
}

function scheduleTransition(contestResponse: Contest): void {
  const now = Date.now()
  const phase = getContestPhase(contestResponse.startsAt, contestResponse.endsAt, now)
  if (phase === 'finished') return
  const nextTime = phase === 'upcoming' ? contestResponse.startsAt : contestResponse.endsAt
  const remaining = Math.min(nextTime.getTime() - now + 100, 2_147_483_647)
  transitionTimer = setTimeout(() => void loadPage(), remaining)
}

async function loadIndividualBoard(contestId: string, signal: AbortSignal): Promise<StandingBoard> {
  const [response, details] = await Promise.all([
    contestsApi.getContestStandings({ contestId }, { signal }),
    getIndividualStandingDetails(contestId, signal).catch((cause: unknown) => {
      if (cause instanceof StandingsPreviewError && cause.status === 404) return null
      throw cause
    })
  ])
  const detailByUser = new Map(
    (details?.users ?? []).map((item: IndividualStandingDetails['users'][number]) => [
      item.userId,
      item
    ])
  )
  return {
    items: response.items.map((item) => {
      const detail = detailByUser.get(item.userId)
      return {
        id: item.userId,
        name: detail?.name || item.userId,
        rank: item.rank,
        score: item.score,
        results: mergeStandingProblemScores(item.problemScores, detail?.results)
      }
    }),
    updatedAt: response.updatedAt,
    isFrozen: details?.isFrozen ?? false,
    frozenAt: details?.frozenAt ? new Date(details.frozenAt) : null,
    problemStats: details?.problemStats,
    firstAccepts: details?.firstAccepts
  }
}

async function loadTeamBoard(contestId: string, signal: AbortSignal): Promise<StandingBoard> {
  const [response, details] = await Promise.all([
    contestsApi.getContestTeamStandings({ contestId }, { signal }),
    getTeamStandingDetails(contestId, signal).catch((cause: unknown) => {
      if (cause instanceof StandingsPreviewError && cause.status === 404) return null
      throw cause
    })
  ])
  const detailByTeam = new Map(
    (details?.teams ?? []).map((item: TeamStandingDetails['teams'][number]) => [item.teamId, item])
  )
  return {
    items: response.items.map((item) => ({
      id: item.teamId,
      name: item.teamName,
      members: item.members.map((member) => member.displayName),
      memberIds: item.members.map((member) => member.userId),
      rank: item.rank,
      score: item.score,
      results: mergeStandingProblemScores(
        item.problemScores,
        detailByTeam.get(item.teamId)?.results
      )
    })),
    updatedAt: response.updatedAt,
    isFrozen: details?.isFrozen ?? false,
    frozenAt: details?.frozenAt ? new Date(details.frozenAt) : null,
    problemStats: details?.problemStats,
    firstAccepts: details?.firstAccepts
  }
}

function loadStandingsBoard(contestId: string, signal: AbortSignal): Promise<StandingBoard> {
  return props.kind === 'individual'
    ? loadIndividualBoard(contestId, signal)
    : props.kind === 'team'
      ? loadTeamBoard(contestId, signal)
      : getExtendedStandingBoard(contestId, signal)
}

async function refreshBoard(): Promise<void> {
  const contestId = contest.value?.id
  if (!contestId || !board.value || isRefreshing.value) return

  const controller = new AbortController()
  requestController = controller
  isRefreshing.value = true
  refreshError.value = ''
  try {
    const response = await loadStandingsBoard(contestId, controller.signal)
    if (!controller.signal.aborted) board.value = response
  } catch {
    if (!controller.signal.aborted) refreshError.value = '順位表を更新できませんでした．'
  } finally {
    if (!controller.signal.aborted) isRefreshing.value = false
  }
}

async function loadPage(): Promise<void> {
  requestController?.abort()
  clearTimeout(transitionTimer)
  const controller = new AbortController()
  requestController = controller
  isRefreshing.value = false
  refreshError.value = ''
  contest.value = null
  problems.value = []
  board.value = null
  selectedStanding.value = null
  notice.value = ''
  error.value = ''
  isLoading.value = true
  const contestId = route.params.contestId

  if (typeof contestId !== 'string' || !contestId) {
    error.value = 'コンテストが見つかりません．URLを確認してください．'
    isLoading.value = false
    return
  }

  try {
    const contestResponse = await contestsApi.getContest(
      { contestId },
      { signal: controller.signal }
    )
    if (controller.signal.aborted) return
    contest.value = contestResponse
    scheduleTransition(contestResponse)
    const now = Date.now()
    const phase = getContestPhase(contestResponse.startsAt, contestResponse.endsAt, now)

    if (props.kind === 'extended' && phase !== 'finished') {
      notice.value = '延長戦順位表はコンテスト終了後に公開されます．'
      return
    }

    if (phase === 'upcoming') {
      notice.value = 'コンテスト開始前のため，まだ順位がありません．'
      return
    }

    if (props.kind !== 'extended' && contestResponse.participationMode !== props.kind) {
      notice.value =
        props.kind === 'team'
          ? 'この大会は個人戦のため，チーム順位表はありません．'
          : 'この大会はチーム戦のため，個人順位表はありません．'
      return
    }

    const needsRoles = contestResponse.visibility === 'private'
    const roles =
      needsRoles && userStore.isAuthenticated
        ? await loadContestViewerRoles(meApi, controller.signal)
        : []
    if (controller.signal.aborted) return
    if (!canViewContestStandings(contestResponse, props.kind, now, roles)) {
      notice.value = userStore.isAuthenticated
        ? 'この順位表を閲覧する権限がありません．'
        : 'この順位表を見るにはログインして大会に参加してください．'
      return
    }

    const [problemResponse, standingsResponse] = await Promise.all([
      contestsApi.listContestProblems({ contestId }, { signal: controller.signal }),
      loadStandingsBoard(contestId, controller.signal)
    ])
    if (controller.signal.aborted) return
    problems.value = [...problemResponse.items].sort((left, right) => left.order - right.order)
    board.value = standingsResponse
  } catch (cause: unknown) {
    if (controller.signal.aborted) return
    const status =
      cause instanceof ResponseError
        ? cause.response.status
        : cause instanceof StandingsPreviewError
          ? cause.status
          : undefined
    if (status === 403 || status === 401) {
      notice.value = 'この順位表を閲覧する権限がありません．'
    } else if (status === 404 && !contest.value) {
      error.value = 'コンテストが見つかりません．URLを確認してください．'
    } else if (status === 404) {
      notice.value = 'この順位表のデータはまだ用意されていません．'
    } else if (status === 409) {
      notice.value = 'この大会では，選択した種類の順位表を表示できません．'
    } else {
      error.value = '順位表を読み込めませんでした．時間をおいて再試行してください．'
    }
  } finally {
    if (!controller.signal.aborted) isLoading.value = false
  }
}

watch(
  () => [route.params.contestId, props.kind],
  () => {
    sort.value = { column: 'total', direction: 'desc' }
    search.value = ''
    page.value = 1
    selectedStanding.value = null
    void loadPage()
  },
  { immediate: true }
)
watch(search, () => {
  page.value = 1
})
watch(totalPages, (count) => {
  if (count > 0 && page.value > count) page.value = count
})
onUnmounted(() => {
  requestController?.abort()
  clearTimeout(transitionTimer)
})
</script>

<template>
  <div class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <div v-if="isLoading && !contest" role="status" class="py-16 text-center text-text-secondary">
      コンテストを読み込み中…
    </div>
    <section v-else-if="error && !contest" class="max-w-2xl py-12">
      <h1 class="fontstyle-ui-title mb-4">順位表を表示できません</h1>
      <p role="alert" class="mb-6 leading-7 text-text-secondary">{{ error }}</p>
      <BorderedButton @click="loadPage">再試行</BorderedButton>
    </section>
    <template v-else-if="contest">
      <ContestHeader :contest="contest" />

      <nav aria-label="順位表の種類" class="border-b border-border-primary py-4">
        <div
          class="inline-flex max-w-full flex-wrap gap-1 rounded border border-border-primary bg-background-secondary p-1"
        >
          <RouterLink
            v-for="entry in standingsTabs"
            :key="entry.name"
            :to="{ name: entry.name, params: { contestId: contest.id } }"
            :aria-current="route.name === entry.name ? 'page' : undefined"
            class="min-w-20 rounded px-4 py-2 text-center text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand-primary"
            :class="
              route.name === entry.name
                ? 'bg-brand-primary font-semibold text-white shadow-sm'
                : 'text-text-secondary hover:bg-background-primary hover:text-text-primary'
            "
            >{{ entry.label }}</RouterLink
          >
        </div>
      </nav>

      <section class="py-8" aria-labelledby="standings-title">
        <div class="mb-5 flex flex-wrap items-center justify-between gap-4">
          <div>
            <h2 id="standings-title" class="text-2xl font-bold text-text-primary">{{ title }}</h2>
          </div>
          <BorderedButton
            icon="refresh"
            :disabled="isLoading || isRefreshing || !board"
            @click="refreshBoard"
            >更新</BorderedButton
          >
        </div>

        <p v-if="refreshError" role="alert" class="mb-4 text-sm text-text-secondary">
          {{ refreshError }}
        </p>

        <p v-if="isLoading" role="status" class="py-12 text-center text-text-secondary">
          順位表を読み込み中…
        </p>
        <div v-else-if="error" class="rounded bg-background-secondary p-5 text-text-secondary">
          <p role="alert" class="mb-4">{{ error }}</p>
          <BorderedButton @click="loadPage">再試行</BorderedButton>
        </div>
        <p
          v-else-if="notice"
          role="status"
          class="rounded bg-background-secondary p-5 text-text-secondary"
        >
          {{ notice }}
        </p>
        <template v-else-if="board">
          <div
            v-if="board.isFrozen"
            class="mb-5 rounded border border-border-primary bg-background-secondary p-4 text-sm text-text-primary"
          >
            <p class="font-semibold">順位表は凍結中です．</p>
            <p v-if="board.frozenAt" class="mt-1 text-text-secondary">
              {{ formatContestDate(board.frozenAt) }}時点の順位と得点を表示しています．
            </p>
            <p class="mt-1 text-text-secondary">
              凍結後の提出は反映待ちとして表示しています．大会終了後に最新の順位を表示します．
            </p>
          </div>
          <p
            v-if="board.items.length === 0"
            class="rounded bg-background-secondary p-5 text-text-secondary"
          >
            {{ kind === 'team' ? '参加チームはまだありません．' : 'まだ順位がありません．' }}
          </p>
          <template v-else>
            <div class="mb-4 max-w-xs">
              <PlainTextbox
                id="standings-search"
                v-model="search"
                :label="searchLabel"
                placeholder="名前またはID"
                type="search"
              />
            </div>
            <p
              v-if="sortedRows.length === 0"
              class="rounded bg-background-secondary p-5 text-text-secondary"
            >
              一致するユーザー・チームはありません．
            </p>
            <ContestStandingsTable
              :kind="kind"
              :problems="problems"
              :problem-stats="board.problemStats ?? {}"
              :rows="pageRows"
              :all-rows="board.items"
              :sort="sort"
              :starts-at="contest.startsAt"
              :first-accepts="board.firstAccepts"
              :highlighted-row-id="highlightedRowId"
              :highlighted-problem-id="highlightedProblemId"
              @sort="changeSort"
              @select-first-solver="selectFirstSolver"
            />
            <nav v-if="totalPages > 1" aria-label="順位表のページ" class="mt-6">
              <SimplePagination v-model="page" :end="totalPages" />
            </nav>
          </template>
        </template>
      </section>
    </template>
  </div>
</template>
