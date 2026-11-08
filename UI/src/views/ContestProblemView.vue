<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import ContestHeader from '@/components/contests/ContestHeader.vue'
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import { useContestQuery } from '@/composables/contests/useContestQuery'
import { contestLocation, useContestId } from '@/router/locations'

const route = useRoute()
const contestId = useContestId()
const contestQuery = useContestQuery(contestId)
const contest = computed(() =>
  contestQuery.view.value.status === 'success' ? contestQuery.view.value.data : undefined
)
const { view, refresh } = useContestProblemsQuery(contest)
const entry = computed(() =>
  view.value.status === 'success'
    ? view.value.data.problems.find((item) => item.problemId === route.params.problemId)
    : undefined
)
</script>

<template>
  <main class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <QueryState :view="contestQuery.view.value" @retry="contestQuery.refresh">
      <template #default="{ data }">
        <ContestHeader :contest="data" />
        <RouterLink
          :to="contestLocation(contestId, 'problems')"
          class="my-6 inline-block text-brand-primary underline"
          >問題一覧へ戻る</RouterLink
        >
        <QueryState :view="view" @retry="refresh">
          <template #default="{ data: page }">
            <p v-if="page.awaitingStart" class="py-6 text-text-secondary">コンテスト開始前です。</p>
            <p v-else-if="!entry" role="alert" class="py-6 text-status-error">
              このコンテストに問題が見つかりませんでした。
            </p>
            <div v-else-if="!entry.problem" role="alert" class="space-y-4 py-6">
              <p>問題を読み込めませんでした。</p>
              <MonochromeButton small @click="refresh">再試行</MonochromeButton>
            </div>
            <article v-else class="min-w-0 space-y-6 py-6">
              <h2 class="fontstyle-ui-subtitle wrap-break-word">
                {{ entry.label }} — {{ entry.problem.title }}
              </h2>
              <MarkdownContent :source="entry.problem.statement" />
              <RouterLink
                :to="{
                  ...contestLocation(contestId, 'submit'),
                  query: { problemId: entry.problemId }
                }"
                class="inline-block rounded-sm bg-brand-primary px-5 py-3 text-white"
                >この問題に提出する</RouterLink
              >
            </article>
          </template>
        </QueryState>
      </template>
    </QueryState>
  </main>
</template>
