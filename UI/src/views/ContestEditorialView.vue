<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import type { Contest } from '@/api/client'
import MarkdownContent from '@/components/MarkdownContent.vue'
import QueryState from '@/components/QueryState.vue'
import { useEditorialQuery } from '@/composables/contests/useEditorialsQuery'
import { contestLocation, useContestId, userLocation } from '@/router/locations'
import { formatContestDate } from '@/utils/contest'

const route = useRoute()
const contestId = useContestId()
const props = defineProps<{ contest: Contest | undefined }>()
const contest = computed(() => props.contest)
const { view, refresh } = useEditorialQuery(
  contest,
  () => (typeof route.params.problemId === 'string' ? route.params.problemId : ''),
  () => (typeof route.params.editorialId === 'string' ? route.params.editorialId : '')
)
const editorial = computed(() =>
  view.value.status === 'success' ? view.value.data.editorial : undefined
)
</script>
<template>
  <RouterLink
    :to="contestLocation(contestId, 'editorials')"
    class="my-6 inline-block text-brand-primary underline"
    >解説一覧へ戻る</RouterLink
  >
  <article class="flex min-w-0 flex-col gap-6 py-6">
    <header v-if="editorial">
      <h2 class="fontstyle-ui-subtitle wrap-break-word">{{ editorial.title }}</h2>
      <div class="mt-2 flex flex-wrap gap-4 text-sm text-text-secondary">
        <span
          >著者
          <RouterLink :to="userLocation(editorial.authorId)" class="underline">{{
            editorial.authorId
          }}</RouterLink></span
        >
        <span v-if="Number.isFinite(Date.parse(editorial.updatedAt))"
          >最終更新 {{ formatContestDate(editorial.updatedAt) }}</span
        >
      </div>
    </header>
    <div v-else-if="view.status === 'pending'" aria-hidden="true" class="h-16">
      <div class="h-8 w-72 max-w-full rounded-sm bg-background-secondary" />
    </div>
    <QueryState :view="view" @retry="refresh">
      <template #default="{ data: page }">
        <p v-if="page.awaitingRelease" class="py-10 text-text-secondary">
          解説はコンテスト終了後に公開されます。
        </p>
        <MarkdownContent v-else-if="page.editorial" :source="page.editorial.body" />
      </template>
    </QueryState>
  </article>
</template>
