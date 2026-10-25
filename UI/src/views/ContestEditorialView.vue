<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import ContestHeader from '@/components/contests/ContestHeader.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestQuery } from '@/composables/contests/useContestQuery'
import { useEditorialQuery } from '@/composables/contests/useEditorialsQuery'
import { contestLocation, useContestId, userLocation } from '@/router/locations'
import { formatContestDate } from '@/utils/contest'

const route = useRoute()
const contestId = useContestId()
const contestQuery = useContestQuery(contestId)
const currentContest = computed(() =>
  contestQuery.view.value.status === 'success' ? contestQuery.view.value.data : undefined
)
const { view, refresh } = useEditorialQuery(
  currentContest,
  () => (typeof route.params.problemId === 'string' ? route.params.problemId : ''),
  () => (typeof route.params.editorialId === 'string' ? route.params.editorialId : '')
)
</script>
<template>
  <main class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <QueryState :view="contestQuery.view.value" @retry="contestQuery.refresh">
      <template #default="{ data: contest }">
        <ContestHeader :contest="contest" />
        <RouterLink
          :to="contestLocation(contestId, 'editorials')"
          class="my-6 inline-block text-brand-primary underline"
          >解説一覧へ戻る</RouterLink
        >
        <QueryState :view="view" @retry="refresh">
          <template #default="{ data: page }">
            <p v-if="page.awaitingRelease" class="py-10 text-text-secondary">
              解説はコンテスト終了後に公開されます。
            </p>
            <article v-else-if="page.editorial" class="flex min-w-0 flex-col gap-6 py-6">
              <header>
                <h2 class="fontstyle-ui-subtitle wrap-break-word">{{ page.editorial.title }}</h2>
                <div class="mt-2 flex flex-wrap gap-4 text-sm text-text-secondary">
                  <span
                    >著者
                    <RouterLink :to="userLocation(page.editorial.authorId)" class="underline">{{
                      page.editorial.authorId
                    }}</RouterLink></span
                  >
                  <span v-if="Number.isFinite(Date.parse(page.editorial.updatedAt))"
                    >最終更新 {{ formatContestDate(page.editorial.updatedAt) }}</span
                  >
                </div>
              </header>
              <MarkdownContent :source="page.editorial.body" />
            </article>
          </template>
        </QueryState>
      </template>
    </QueryState>
  </main>
</template>
