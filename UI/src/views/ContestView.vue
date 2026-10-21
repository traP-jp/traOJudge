<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { ContestsApi } from '@/api/generated/contest/apis/ContestsApi'
import type { GetContest200Response as Contest } from '@/api/generated/contest/models/GetContest200Response'
import { Configuration, ResponseError } from '@/api/generated/contest/runtime'
import MarkdownContent from '@/components/MarkdownContent.vue'
import BorderedButton from '@/components/Controls/BorderedButton.vue'
import ContestHeader from '@/components/Navigations/ContestHeader.vue'
import { formatContestDate, formatContestDuration } from '@/utils/contest'

const route = useRoute()
const contest = ref<Contest | null>(null)
const isLoading = ref(true)
const error = ref('')
const api = new ContestsApi(
  new Configuration({
    basePath: import.meta.env.VITE_CONTEST_API_BASE_URL || '/api/v1',
    credentials: 'include'
  })
)
let requestController: AbortController | undefined

async function loadContest(): Promise<void> {
  requestController?.abort()
  const controller = new AbortController()
  requestController = controller
  contest.value = null
  error.value = ''
  isLoading.value = true
  const contestId = route.params.contestId

  if (typeof contestId !== 'string' || !contestId) {
    error.value = 'コンテストが見つかりません．URLを確認してください．'
    isLoading.value = false
    return
  }

  try {
    const response = await api.getContest({ contestId }, { signal: controller.signal })
    if (controller.signal.aborted) return
    if (
      !Number.isFinite(response.startsAt.getTime()) ||
      !Number.isFinite(response.endsAt.getTime()) ||
      response.endsAt <= response.startsAt
    ) {
      throw new Error('Invalid contest schedule')
    }
    contest.value = response
  } catch (cause: unknown) {
    if (controller.signal.aborted) return
    const status = cause instanceof ResponseError ? cause.response.status : undefined
    if (status === 404) {
      error.value = 'コンテストが見つかりません．URLを確認してください．'
    } else if (status === 401 || status === 403) {
      error.value = 'このコンテストを閲覧する権限がありません．ログイン状態を確認してください．'
    } else {
      error.value = 'コンテストを読み込めませんでした．時間をおいて再試行してください．'
    }
  } finally {
    if (!controller.signal.aborted) isLoading.value = false
  }
}

watch(() => route.params.contestId, loadContest, { immediate: true })
onUnmounted(() => requestController?.abort())
</script>

<template>
  <div class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <div v-if="isLoading" role="status" class="py-16 text-center text-text-secondary">
      コンテストを読み込み中…
    </div>
    <section v-else-if="error" aria-labelledby="contest-error" class="max-w-2xl py-12">
      <h1 id="contest-error" class="fontstyle-ui-title mb-4">コンテストを表示できません</h1>
      <p role="alert" class="mb-6 leading-7 text-text-secondary">{{ error }}</p>
      <BorderedButton
        class="focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
        @click="loadContest"
        >再試行</BorderedButton
      >
    </section>
    <template v-else-if="contest">
      <ContestHeader :contest="contest" />

      <div class="flex min-w-0 flex-col gap-10 py-8 lg:flex-row lg:gap-12">
        <article aria-labelledby="contest-overview" class="min-w-0 flex-1">
          <h2 id="contest-overview" class="fontstyle-ui-subtitle mb-6">コンテスト概要</h2>
          <MarkdownContent v-if="contest.description?.trim()" :source="contest.description" />
          <p v-else class="leading-7 text-text-secondary">
            コンテストの概要はまだ掲載されていません．
          </p>
        </article>

        <aside aria-label="開催情報とルール" class="w-full shrink-0 lg:w-72">
          <section
            aria-labelledby="contest-schedule"
            class="border-t border-border-primary pt-6 lg:border-t-0 lg:pt-0"
          >
            <h2 id="contest-schedule" class="fontstyle-ui-subtitle mb-5">開催情報</h2>
            <dl class="space-y-5 text-sm">
              <div>
                <dt class="mb-1 text-text-secondary">開始日時</dt>
                <dd class="font-medium tabular-nums text-text-primary">
                  <time :datetime="contest.startsAt.toISOString()">{{
                    formatContestDate(contest.startsAt)
                  }}</time>
                </dd>
              </div>
              <div>
                <dt class="mb-1 text-text-secondary">終了日時</dt>
                <dd class="font-medium tabular-nums text-text-primary">
                  <time :datetime="contest.endsAt.toISOString()">{{
                    formatContestDate(contest.endsAt)
                  }}</time>
                </dd>
              </div>
              <div>
                <dt class="mb-1 text-text-secondary">開催時間</dt>
                <dd class="font-medium tabular-nums text-text-primary">
                  {{ formatContestDuration(contest.endsAt.getTime() - contest.startsAt.getTime()) }}
                </dd>
              </div>
            </dl>
            <p class="mt-4 text-xs text-text-secondary">日時は日本標準時（JST）です</p>
          </section>

          <section aria-labelledby="contest-rules" class="mt-8 border-t border-border-primary pt-6">
            <h2 id="contest-rules" class="fontstyle-ui-subtitle mb-5">ルール</h2>
            <dl class="space-y-5 text-sm">
              <div>
                <dt class="mb-1 text-text-secondary">同点時の順位</dt>
                <dd class="font-medium text-text-primary">
                  {{ contest.rules.tieBreak === 'icpc' ? 'ICPC方式' : 'AtCoder方式' }}
                </dd>
              </div>
              <div>
                <dt class="mb-1 text-text-secondary">誤答ペナルティ</dt>
                <dd class="font-medium tabular-nums text-text-primary">
                  {{
                    contest.rules.penaltyMinutes === 0
                      ? 'なし'
                      : `${contest.rules.penaltyMinutes}分`
                  }}
                </dd>
              </div>
            </dl>
          </section>
        </aside>
      </div>
    </template>
  </div>
</template>
