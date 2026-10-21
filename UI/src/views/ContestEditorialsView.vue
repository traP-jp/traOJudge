<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { ContestsApi } from '@/api/generated/contest/apis/ContestsApi'
import { EditorialsApi } from '@/api/generated/contest/apis/EditorialsApi'
import { MeApi } from '@/api/generated/contest/apis/MeApi'
import { ProblemsApi } from '@/api/generated/contest/apis/ProblemsApi'
import type { GetContest200Response } from '@/api/generated/contest/models/GetContest200Response'
import type { ListContestProblems200ResponseItemsInner } from '@/api/generated/contest/models/ListContestProblems200ResponseItemsInner'
import type { ListProblemEditorials200ResponseItemsInner } from '@/api/generated/contest/models/ListProblemEditorials200ResponseItemsInner'
import { Configuration, ResponseError } from '@/api/generated/contest/runtime'
import BorderedButton from '@/components/Controls/BorderedButton.vue'
import ContestHeader from '@/components/Navigations/ContestHeader.vue'
import { useUserStore } from '@/stores/user'
import {
  canViewEditorial,
  getEarlyEditorialAccess,
  loadMyEditorialRoles
} from '@/utils/editorialAccess'

type Editorial = ListProblemEditorials200ResponseItemsInner
type ContestProblem = ListContestProblems200ResponseItemsInner
type ProblemEditorials = ContestProblem & {
  title: string | null
  editorials: Editorial[]
  error: boolean
}

const route = useRoute()
const userStore = useUserStore()
const contest = ref<GetContest200Response | null>(null)
const problems = ref<ProblemEditorials[]>([])
const isLoading = ref(true)
const isAwaitingRelease = ref(false)
const error = ref('')
const config = new Configuration({
  basePath: import.meta.env.VITE_CONTEST_API_BASE_URL || '/api/v1',
  credentials: 'include'
})
const contestsApi = new ContestsApi(config)
const editorialsApi = new EditorialsApi(config)
const meApi = new MeApi(config)
const problemsApi = new ProblemsApi(config)
let requestController: AbortController | undefined
let releaseTimer: ReturnType<typeof setTimeout> | undefined

function scheduleRelease(endsAt: Date): void {
  const remaining = endsAt.getTime() - Date.now()
  if (remaining <= 0) return
  releaseTimer = setTimeout(
    () => {
      if (Date.now() >= endsAt.getTime()) void loadPage()
      else scheduleRelease(endsAt)
    },
    Math.min(remaining + 100, 2_147_483_647)
  )
}

async function listAllEditorials(
  problemId: string,
  signal: AbortSignal,
  canView: (editorial: Editorial) => boolean
): Promise<Editorial[]> {
  const items: Editorial[] = []
  const seenCursors = new Set<string>()
  let cursor: string | undefined

  do {
    const page = await editorialsApi.listProblemEditorials(
      { problemId, limit: 100, cursor },
      { signal }
    )
    items.push(
      ...page.items.filter((editorial) => editorial.problemId === problemId && canView(editorial))
    )
    // OpenAPI Generatorはnullableなcursorを空のモデルにするため，実際の値を確認する．
    const nextCursor: unknown = page.nextCursor
    if (nextCursor != null && typeof nextCursor !== 'string') {
      throw new Error('Invalid editorial cursor')
    }
    cursor = nextCursor ?? undefined
    if (cursor && seenCursors.has(cursor)) throw new Error('Repeated editorial cursor')
    if (cursor) seenCursors.add(cursor)
  } while (cursor)

  return items
}

async function loadProblem(
  problem: ContestProblem,
  signal: AbortSignal,
  canView: (editorial: Editorial) => boolean
): Promise<ProblemEditorials> {
  const [problemResult, editorialsResult] = await Promise.allSettled([
    problemsApi.getProblem({ problemId: problem.problemId }, { signal }),
    listAllEditorials(problem.problemId, signal, canView)
  ])

  return {
    ...problem,
    title: problemResult.status === 'fulfilled' ? problemResult.value.title : null,
    editorials: editorialsResult.status === 'fulfilled' ? editorialsResult.value : [],
    error: editorialsResult.status === 'rejected'
  }
}

async function getViewerRoles(
  hasFinished: boolean,
  signal: AbortSignal
): Promise<Awaited<ReturnType<typeof loadMyEditorialRoles>>> {
  if (!userStore.isAuthenticated) return []
  try {
    return await loadMyEditorialRoles(meApi, signal)
  } catch (cause: unknown) {
    if (hasFinished) return []
    throw cause
  }
}

async function loadPage(): Promise<void> {
  requestController?.abort()
  clearTimeout(releaseTimer)
  const controller = new AbortController()
  requestController = controller
  contest.value = null
  problems.value = []
  isAwaitingRelease.value = false
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
    const hasFinished = Date.now() >= contestResponse.endsAt.getTime()
    if (!hasFinished) scheduleRelease(contestResponse.endsAt)

    const roles = await getViewerRoles(hasFinished, controller.signal)
    if (controller.signal.aborted) return
    const access = getEarlyEditorialAccess(roles, contestId, userStore.userId)
    if (!hasFinished && !userStore.isAuthenticated) {
      isAwaitingRelease.value = true
      return
    }

    const problemResponse = await contestsApi.listContestProblems(
      { contestId },
      { signal: controller.signal }
    )
    if (controller.signal.aborted) return
    const orderedProblems = [...problemResponse.items].sort(
      (a, b) => a.order - b.order || a.label.localeCompare(b.label)
    )
    const canView = (editorial: Editorial): boolean =>
      canViewEditorial(editorial, hasFinished, access)
    const loadedProblems = await Promise.all(
      orderedProblems.map((problem) => loadProblem(problem, controller.signal, canView))
    )
    if (controller.signal.aborted) return
    problems.value = loadedProblems
    isAwaitingRelease.value =
      !hasFinished &&
      !access.isOrganizer &&
      loadedProblems.every((problem) => !problem.error && problem.editorials.length === 0)
  } catch (cause: unknown) {
    if (controller.signal.aborted) return
    const status = cause instanceof ResponseError ? cause.response.status : undefined
    if (status === 404) {
      error.value = 'コンテストが見つかりません．URLを確認してください．'
    } else if (status === 401 || status === 403) {
      error.value = 'このコンテストの解説を閲覧する権限がありません．'
    } else {
      error.value = '解説一覧を読み込めませんでした．時間をおいて再試行してください．'
    }
  } finally {
    if (!controller.signal.aborted) isLoading.value = false
  }
}

watch(() => route.params.contestId, loadPage, { immediate: true })
onUnmounted(() => {
  requestController?.abort()
  clearTimeout(releaseTimer)
})
</script>

<template>
  <main class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <div v-if="isLoading" role="status" class="py-16 text-center text-text-secondary">
      解説一覧を読み込み中…
    </div>
    <section v-else-if="error" aria-labelledby="editorials-error" class="max-w-2xl py-12">
      <h1 id="editorials-error" class="fontstyle-ui-title mb-4">解説一覧を表示できません</h1>
      <p role="alert" class="mb-6 leading-7 text-text-secondary">{{ error }}</p>
      <BorderedButton @click="loadPage">再試行</BorderedButton>
    </section>
    <template v-else-if="contest">
      <ContestHeader :contest="contest" />
      <section aria-labelledby="contest-editorials-heading" class="max-w-5xl py-8">
        <h2 id="contest-editorials-heading" class="fontstyle-ui-subtitle mb-2">解説一覧</h2>
        <p v-if="isAwaitingRelease" class="py-10 leading-7 text-text-secondary">
          解説はコンテスト終了後に公開されます．
        </p>
        <p v-else-if="problems.length === 0" class="py-10 leading-7 text-text-secondary">
          このコンテストには問題が登録されていません．
        </p>
        <ol v-else class="divide-y divide-border-primary">
          <li v-for="problem in problems" :key="problem.problemId" class="py-8">
            <div class="mb-5 flex min-w-0 items-start gap-3">
              <span
                class="shrink-0 rounded bg-background-secondary px-3 py-1 text-sm font-semibold text-text-primary"
                >{{ problem.label }}</span
              >
              <h2 class="min-w-0 break-words text-lg font-semibold text-text-primary">
                {{ problem.title || `問題 ${problem.label}` }}
              </h2>
            </div>
            <p v-if="problem.error" role="alert" class="leading-7 text-text-secondary">
              この問題の解説を読み込めませんでした．再試行してください．
            </p>
            <p v-else-if="problem.editorials.length === 0" class="leading-7 text-text-secondary">
              解説はまだ掲載されていません．
            </p>
            <ul v-else class="space-y-3">
              <li v-for="editorial in problem.editorials" :key="editorial.id">
                <RouterLink
                  :to="{
                    path: `/problems/${encodeURIComponent(problem.problemId)}/editorials/${encodeURIComponent(editorial.id)}`,
                    query: { contestId: contest.id }
                  }"
                  class="break-words text-brand-primary underline underline-offset-2 hover:text-brand-secondary focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
                  >{{ editorial.title }}</RouterLink
                >
              </li>
            </ul>
          </li>
        </ol>
      </section>
    </template>
  </main>
</template>
