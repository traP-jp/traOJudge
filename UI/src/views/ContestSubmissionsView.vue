<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  Configuration,
  ContestsApi,
  MeApi,
  ProblemsApi,
  type Contest,
  type Language
} from '@/api/generated'
import ContestHeader from '@/components/Navigations/ContestHeader.vue'
import ContestSubmissionsTable from '@/components/ContestSubmissionsTable.vue'
import BorderedButton from '@/components/Controls/BorderedButton.vue'
import { useUserStore } from '@/stores/user'
import { apiErrorMessage } from '@/utils/apiError'
import { loadContestViewerRoles } from '@/utils/contestStandingsAccess'
import type {
  ContestSubmissionProblem,
  ContestSubmissionScope,
  ContestSubmissionTeam
} from '@/types/contestSubmissions'

const route = useRoute()
const router = useRouter()
const userStore = useUserStore()
const config = new Configuration({
  basePath:
    import.meta.env.VITE_CONTEST_API_BASE_URL || import.meta.env.VITE_API_BASE_URL || '/api/v1',
  credentials: 'include'
})
const contestsApi = new ContestsApi(config)
const problemsApi = new ProblemsApi(config)
const meApi = new MeApi(config)
const contest = ref<Contest>()
const problems = ref<ContestSubmissionProblem[]>([])
const languages = ref<Language[]>([])
const teams = ref<ContestSubmissionTeam[]>([])
const isSetter = ref(false)
const now = ref(Date.now())
const isLoading = ref(true)
const error = ref('')
const teamError = ref('')
let controller: AbortController | undefined
let transition: ReturnType<typeof setTimeout> | undefined

const hasFinished = computed(() => !!contest.value && now.value >= contest.value.endsAt.getTime())
const canViewAll = computed(() => hasFinished.value || isSetter.value)
const scope = computed<ContestSubmissionScope>(() => {
  if (route.query.scope === 'mine' || route.query.scope === 'team' || route.query.scope === 'all')
    return route.query.scope
  return canViewAll.value || !userStore.isAuthenticated ? 'all' : 'mine'
})
const problemId = computed(() =>
  typeof route.query.problemId === 'string' ? route.query.problemId : ''
)
const availableTeams = computed(() =>
  canViewAll.value
    ? teams.value
    : teams.value.filter((team) =>
        team.members.some((member) => member.userId === userStore.userId)
      )
)
const teamId = computed(() =>
  typeof route.query.teamId === 'string' ? route.query.teamId : (availableTeams.value[0]?.id ?? '')
)
const notice = computed(() => {
  if (problemId.value && !problems.value.some((problem) => problem.problemId === problemId.value))
    return '指定された問題はこのコンテストに登録されていません．'
  if (scope.value === 'all' && !canViewAll.value)
    return '全体の提出一覧はコンテスト終了後に公開されます．開催中は自分または自分のチームの提出を確認できます．'
  if (scope.value === 'mine' && !userStore.isAuthenticated)
    return '自分の提出を見るにはログインしてください．'
  if (scope.value !== 'team') return ''
  if (contest.value?.participationMode !== 'team') return 'このコンテストは個人戦です．'
  if (!canViewAll.value && !userStore.isAuthenticated)
    return 'チームの提出を見るにはログインしてください．'
  if (teamError.value) return teamError.value
  if (!availableTeams.value.length)
    return canViewAll.value ? '参加チームはまだありません．' : '所属チームを確認できませんでした．'
  if (!availableTeams.value.some((team) => team.id === teamId.value))
    return 'このチームの提出は表示できません．'
  return ''
})
const tabs = computed(() => [
  { id: 'all', label: 'すべて' },
  { id: 'mine', label: '自分の提出' },
  ...(contest.value?.participationMode === 'team' ? [{ id: 'team', label: 'チーム' }] : [])
])

function setFilter(key: 'scope' | 'problemId' | 'teamId', value: string): void {
  void router.replace({ query: { ...route.query, [key]: value || undefined } })
}

function selectFilter(key: 'problemId' | 'teamId', event: Event): void {
  setFilter(key, (event.target as HTMLSelectElement).value)
}

async function loadPage(): Promise<void> {
  controller?.abort()
  clearTimeout(transition)
  const request = new AbortController()
  controller = request
  contest.value = undefined
  problems.value = []
  teams.value = []
  languages.value = []
  isSetter.value = false
  error.value = ''
  teamError.value = ''
  isLoading.value = true
  const contestId = String(route.params.contestId)
  try {
    const value = await contestsApi.getContest({ contestId }, { signal: request.signal })
    const [problemResponse, languageResponse, roles] = await Promise.all([
      contestsApi.listContestProblems({ contestId }, { signal: request.signal }),
      problemsApi.listLanguages({ signal: request.signal }),
      userStore.isAuthenticated
        ? loadContestViewerRoles(meApi, request.signal)
        : Promise.resolve([])
    ])
    const problemRows = await Promise.all(
      problemResponse.items.map(async (problem) => {
        const detail = await problemsApi
          .getProblem({ problemId: problem.problemId }, { signal: request.signal })
          .catch(() => null)
        return {
          ...problem,
          title: detail?.title ?? `問題 ${problem.label}`,
          canOpen: detail !== null
        }
      })
    )
    let teamRows: ContestSubmissionTeam[] = []
    let failedTeams = false
    if (value.participationMode === 'team') {
      // 順位APIに含まれるチームと現在のメンバーを選択肢に使う．一覧の取得権限は提出APIでも検証する．
      const board = await contestsApi
        .getContestTeamStandings({ contestId }, { signal: request.signal })
        .catch(() => null)
      teamRows =
        board?.items.map((team) => ({
          id: team.teamId,
          name: team.teamName,
          members: team.members
        })) ?? []
      failedTeams = board === null
    }
    if (request.signal.aborted) return
    contest.value = value
    problems.value = problemRows.sort((a, b) => a.order - b.order)
    languages.value = languageResponse.items
    teams.value = teamRows
    isSetter.value = roles.some(
      (role) =>
        role.role === 'contest-setter' &&
        role.scope.kind === 'contest' &&
        role.scope.contestId === contestId
    )
    teamError.value = failedTeams
      ? 'チーム情報を取得できませんでした．ページを再読み込みしてください．'
      : ''
    now.value = Date.now()
    if (value.endsAt.getTime() > now.value) {
      transition = setTimeout(
        () => void loadPage(),
        Math.min(value.endsAt.getTime() - now.value + 100, 2_147_483_647)
      )
    }
  } catch (cause) {
    const message = await apiErrorMessage(cause)
    if (!request.signal.aborted) error.value = message
  } finally {
    if (!request.signal.aborted) isLoading.value = false
  }
}

watch(
  () => [route.params.contestId, userStore.userId],
  () => void loadPage(),
  { immediate: true }
)
onUnmounted(() => {
  controller?.abort()
  clearTimeout(transition)
})
</script>

<template>
  <main class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <p v-if="isLoading" role="status" class="py-16 text-center text-text-secondary">
      コンテストを読み込み中…
    </p>
    <section v-else-if="error" class="space-y-4 py-12">
      <h1 class="fontstyle-ui-title">提出一覧を表示できません</h1>
      <p role="alert">{{ error }}</p>
      <BorderedButton @click="loadPage">再試行</BorderedButton>
    </section>
    <template v-else-if="contest">
      <ContestHeader :contest="contest" />
      <section class="py-8" aria-labelledby="submissions-title">
        <h2 id="submissions-title" class="mb-6 text-2xl font-bold">提出一覧</h2>
        <div class="mb-6 flex flex-wrap items-end justify-between gap-4">
          <nav
            aria-label="提出の表示範囲"
            class="inline-flex gap-1 rounded border border-border-primary bg-background-secondary p-1"
          >
            <button
              v-for="tab in tabs"
              :key="tab.id"
              type="button"
              :aria-pressed="scope === tab.id"
              class="min-h-10 rounded px-4 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand-primary"
              :class="
                scope === tab.id
                  ? 'bg-brand-primary font-semibold text-white'
                  : 'text-text-secondary hover:bg-background-primary'
              "
              @click="setFilter('scope', tab.id)"
            >
              {{ tab.label }}
            </button>
          </nav>
          <div class="flex flex-wrap gap-4">
            <div v-if="scope === 'team' && availableTeams.length">
              <label for="submission-team" class="mb-1 block text-sm text-text-secondary"
                >チーム</label
              >
              <select
                id="submission-team"
                :value="teamId"
                class="min-h-10 rounded border border-border-secondary bg-background-primary px-3 text-sm"
                @change="selectFilter('teamId', $event)"
              >
                <option v-for="team in availableTeams" :key="team.id" :value="team.id">
                  {{ team.name }}
                </option>
              </select>
            </div>
            <div>
              <label for="submission-problem" class="mb-1 block text-sm text-text-secondary"
                >問題</label
              >
              <select
                id="submission-problem"
                :value="problemId"
                class="min-h-10 max-w-full rounded border border-border-secondary bg-background-primary px-3 text-sm"
                @change="selectFilter('problemId', $event)"
              >
                <option value="">すべての問題</option>
                <option
                  v-for="problem in problems"
                  :key="problem.problemId"
                  :value="problem.problemId"
                >
                  {{ problem.label }} — {{ problem.title }}
                </option>
              </select>
            </div>
          </div>
        </div>
        <div
          v-if="notice"
          role="status"
          class="space-y-3 rounded bg-background-secondary p-5 text-text-secondary"
        >
          <p>{{ notice }}</p>
          <RouterLink
            v-if="!userStore.isAuthenticated"
            :to="{ path: '/login', query: { redirect: route.fullPath } }"
            class="inline-block text-brand-primary underline"
            >ログイン</RouterLink
          >
        </div>
        <ContestSubmissionsTable
          v-else
          :contest-id="contest.id"
          :scope="scope"
          :viewer-id="userStore.userId"
          :team-id="teamId"
          :problem-id="problemId"
          :problems="problems"
          :teams="teams"
          :languages="languages"
          :can-view-all="canViewAll"
          :team-contest="contest.participationMode === 'team'"
        />
      </section>
    </template>
  </main>
</template>
