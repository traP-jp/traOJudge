<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import type { Contest } from '@/api/client'
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
</script>

<template>
  <section class="max-w-4xl space-y-6 py-6 sm:py-8">
    <RouterLink
      :to="contestLocation(contestId, 'problems')"
      class="inline-block text-brand-primary underline"
      >問題一覧へ戻る</RouterLink
    >
    <article class="min-w-0 space-y-8">
      <h2 v-if="entry?.problem" class="contest-page-heading wrap-break-word">
        {{ entry.label }} — {{ entry.problem.title }}
      </h2>
      <div
        v-else-if="view.status === 'pending'"
        aria-hidden="true"
        class="h-7 w-72 max-w-full rounded-sm bg-background-secondary sm:h-8"
      />
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
            <RouterLink
              :to="{
                ...contestLocation(contestId, 'submit'),
                query: { problemId: entry.problemId }
              }"
              class="inline-block rounded-sm bg-brand-primary px-5 py-3 text-white"
              >この問題に提出する</RouterLink
            >
          </template>
        </template>
      </QueryState>
    </article>
  </section>
</template>
