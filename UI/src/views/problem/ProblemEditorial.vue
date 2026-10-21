<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { ContestsApi } from '@/api/generated/contest/apis/ContestsApi'
import { EditorialsApi } from '@/api/generated/contest/apis/EditorialsApi'
import { MeApi } from '@/api/generated/contest/apis/MeApi'
import type { GetEditorial200Response } from '@/api/generated/contest/models/GetEditorial200Response'
import { Configuration, ResponseError } from '@/api/generated/contest/runtime'
import BorderedButton from '@/components/Controls/BorderedButton.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { useUserStore } from '@/stores/user'
import { getEarlyEditorialAccess, loadMyEditorialRoles } from '@/utils/editorialAccess'

const route = useRoute()
const userStore = useUserStore()
const editorial = ref<GetEditorial200Response | null>(null)
const isLoading = ref(true)
const error = ref('')
const config = new Configuration({
  basePath: import.meta.env.VITE_CONTEST_API_BASE_URL || '/api/v1',
  credentials: 'include'
})
const contestsApi = new ContestsApi(config)
const editorialsApi = new EditorialsApi(config)
const meApi = new MeApi(config)
let requestController: AbortController | undefined

const updatedAt = computed(() => {
  const date = editorial.value?.updatedAt
  if (!date || !Number.isFinite(date.getTime())) return ''
  return new Intl.DateTimeFormat('ja-JP', {
    dateStyle: 'medium',
    timeStyle: 'short',
    timeZone: 'Asia/Tokyo'
  }).format(date)
})

type ContestAccessCheck = { error: string; verifyAuthor: boolean }

async function checkContestAccess(
  contestId: unknown,
  problemId: string,
  editorialId: string,
  signal: AbortSignal
): Promise<ContestAccessCheck> {
  if (contestId === undefined) return { error: '', verifyAuthor: false }
  if (typeof contestId !== 'string' || !contestId) {
    return { error: '解説が見つかりません．URLを確認してください．', verifyAuthor: false }
  }
  const [contest, problems] = await Promise.all([
    contestsApi.getContest({ contestId }, { signal }),
    contestsApi.listContestProblems({ contestId }, { signal })
  ])
  if (!problems.items.some((problem) => problem.problemId === problemId)) {
    return { error: '解説が見つかりません．URLを確認してください．', verifyAuthor: false }
  }
  if (Date.now() >= contest.endsAt.getTime()) return { error: '', verifyAuthor: false }
  const roles = await loadMyEditorialRoles(meApi, signal)
  const { isOrganizer, authoredEditorialIds } = getEarlyEditorialAccess(roles, contestId)
  if (isOrganizer || authoredEditorialIds.has(editorialId)) {
    return { error: '', verifyAuthor: false }
  }
  return { error: '', verifyAuthor: true }
}

async function loadEditorial(): Promise<void> {
  requestController?.abort()
  const controller = new AbortController()
  requestController = controller
  editorial.value = null
  error.value = ''
  isLoading.value = true
  const editorialId = route.params.editorialId
  const problemId = route.params.id
  const contestId = route.query.contestId

  if (typeof editorialId !== 'string' || !editorialId || typeof problemId !== 'string') {
    error.value = '解説が見つかりません．URLを確認してください．'
    isLoading.value = false
    return
  }

  try {
    const access = await checkContestAccess(contestId, problemId, editorialId, controller.signal)
    if (controller.signal.aborted) return
    if (access.error) {
      error.value = access.error
      return
    }
    const response = await editorialsApi.getEditorial(
      { editorialId },
      { signal: controller.signal }
    )
    if (controller.signal.aborted) return
    if (response.problemId !== problemId) {
      error.value = '解説が見つかりません．URLを確認してください．'
      return
    }
    if (access.verifyAuthor && response.authorId !== userStore.userId) {
      error.value = '解説はコンテスト終了後に公開されます．'
      return
    }
    editorial.value = response
  } catch (cause: unknown) {
    if (controller.signal.aborted) return
    const status = cause instanceof ResponseError ? cause.response.status : undefined
    if (status === 404) {
      error.value = '解説が見つかりません．URLを確認してください．'
    } else if (status === 401 || status === 403) {
      error.value = 'この解説を閲覧する権限がありません．'
    } else {
      error.value = '解説を読み込めませんでした．時間をおいて再試行してください．'
    }
  } finally {
    if (!controller.signal.aborted) isLoading.value = false
  }
}

watch(() => [route.params.id, route.params.editorialId, route.query.contestId], loadEditorial, {
  immediate: true
})
onUnmounted(() => requestController?.abort())
</script>

<template>
  <article class="flex min-h-header-offset min-w-0 flex-1 flex-col items-start gap-6 py-6">
    <div v-if="isLoading" role="status" class="text-text-secondary">解説を読み込み中…</div>
    <section v-else-if="error" aria-labelledby="editorial-error" class="py-8">
      <h1 id="editorial-error" class="fontstyle-ui-subtitle mb-4">解説を表示できません</h1>
      <p role="alert" class="mb-6 text-text-secondary">{{ error }}</p>
      <BorderedButton @click="loadEditorial">再試行</BorderedButton>
    </section>
    <template v-else-if="editorial">
      <header class="flex flex-col items-start gap-2 self-stretch">
        <h1 class="fontstyle-ui-subtitle break-words text-text-primary">{{ editorial.title }}</h1>
        <div class="flex flex-wrap items-center gap-x-5 gap-y-1 text-sm text-text-secondary">
          <div>
            著者
            <RouterLink
              :to="`/users/${encodeURIComponent(editorial.authorId)}`"
              class="ml-1 underline underline-offset-2 hover:text-brand-primary"
              >{{ editorial.authorId }}</RouterLink
            >
          </div>
          <div v-if="updatedAt">最終更新 {{ updatedAt }}</div>
        </div>
      </header>
      <MarkdownContent class="w-full" :source="editorial.body" />
    </template>
  </article>
</template>
