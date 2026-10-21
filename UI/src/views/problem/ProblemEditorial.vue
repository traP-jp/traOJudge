<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { EditorialsApi } from '@/api/generated/contest/apis/EditorialsApi'
import type { GetEditorial200Response } from '@/api/generated/contest/models/GetEditorial200Response'
import { Configuration, ResponseError } from '@/api/generated/contest/runtime'
import BorderedButton from '@/components/Controls/BorderedButton.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'

const route = useRoute()
const editorial = ref<GetEditorial200Response | null>(null)
const isLoading = ref(true)
const error = ref('')
const editorialsApi = new EditorialsApi(
  new Configuration({
    basePath: import.meta.env.VITE_CONTEST_API_BASE_URL || '/api/v1',
    credentials: 'include'
  })
)
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

async function loadEditorial(): Promise<void> {
  requestController?.abort()
  const controller = new AbortController()
  requestController = controller
  editorial.value = null
  error.value = ''
  isLoading.value = true
  const editorialId = route.params.editorialId
  const problemId = route.params.id

  if (typeof editorialId !== 'string' || !editorialId || typeof problemId !== 'string') {
    error.value = '解説が見つかりません．URLを確認してください．'
    isLoading.value = false
    return
  }

  try {
    const response = await editorialsApi.getEditorial(
      { editorialId },
      { signal: controller.signal }
    )
    if (controller.signal.aborted) return
    if (response.problemId !== problemId) {
      error.value = '解説が見つかりません．URLを確認してください．'
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

watch(() => [route.params.id, route.params.editorialId], loadEditorial, { immediate: true })
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
