<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import type { Contest } from '@/api/client'
import ContestProblemSwitcher from '@/components/contests/ContestProblemSwitcher.vue'
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import { contestLocation, useContestId } from '@/router/locations'

const route = useRoute()
const contestId = useContestId()
const props = defineProps<{ contest: Contest | undefined }>()
const contest = computed(() => props.contest)
const { view, refresh } = useContestProblemsQuery(contest)
const entry = computed(() =>
  view.value.status === 'success'
    ? view.value.data.problems.find((item) => item.problemId === route.params.problemId)
    : undefined
)
const problemOptions = computed(() =>
  view.value.status === 'success'
    ? view.value.data.problems.map((problem) => ({
        id: problem.problemId,
        name: `${problem.label} — ${problem.problem?.title || '問題'}`
      }))
    : []
)
const submitLocation = computed(() => ({
  ...contestLocation(contestId.value, 'submit'),
  query: { problemId: entry.value?.problemId }
}))
const submitClass =
  'inline-flex h-10 shrink-0 items-center justify-center rounded-sm border border-brand-primary bg-brand-primary px-6 fontstyle-ui-control-strong text-white hover:border-brand-secondary hover:bg-brand-secondary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary'
</script>

<template>
  <section class="max-w-4xl space-y-6 py-6 sm:py-8">
    <RouterLink
      :to="contestLocation(contestId, 'problems')"
      class="inline-flex h-10 items-center rounded-sm border border-text-primary px-6 fontstyle-ui-control-strong text-text-primary hover:bg-background-secondary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
      >問題一覧へ戻る</RouterLink
    >
    <article class="min-w-0 space-y-8">
      <header v-if="entry?.problem" class="flex flex-col gap-4 sm:flex-row sm:items-start">
        <ContestProblemSwitcher
          :contest-id="contestId"
          :problem-id="entry.problemId"
          :problems="problemOptions"
          class="sm:flex-1"
        />
        <RouterLink :to="submitLocation" :class="submitClass" class="self-start"
          >この問題に提出する</RouterLink
        >
      </header>
      <div v-else-if="view.status === 'pending'" aria-hidden="true" class="min-h-24 sm:min-h-10">
        <div class="h-10 w-72 max-w-full rounded-sm bg-background-secondary" />
      </div>
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
          <template v-else>
            <MarkdownContent
              :source="entry.problem.statement"
              :heading-level="3"
              class="contest-markdown"
            />
            <RouterLink :to="submitLocation" :class="submitClass">この問題に提出する</RouterLink>
          </template>
        </template>
      </QueryState>
    </article>
  </section>
</template>
