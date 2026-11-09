<script setup lang="ts">
import { computed } from 'vue'

import type { Contest } from '@/api/client'
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import { contestProblemLocation, useContestId } from '@/router/locations'

const contestId = useContestId()
const props = defineProps<{ contest: Contest | undefined }>()
const contest = computed(() => props.contest)
const { view, refresh } = useContestProblemsQuery(contest)
</script>

<template>
  <section class="max-w-4xl py-6 sm:py-8">
    <h2 class="mb-6 contest-page-heading">問題一覧</h2>
    <QueryState :view="view" @retry="refresh">
      <template #default="{ data: page }">
        <p v-if="page.awaitingStart" class="text-text-secondary">コンテスト開始前です。</p>
        <p v-else-if="page.problems.length === 0" class="text-text-secondary">
          問題はまだ登録されていません。
        </p>
        <ol v-else class="divide-y divide-border-primary">
          <li
            v-for="entry in page.problems"
            :key="entry.problemId"
            class="flex items-start gap-3 py-4"
          >
            <span
              class="min-w-9 shrink-0 rounded-sm bg-background-secondary px-3 py-1 text-center text-sm font-semibold"
              >{{ entry.label }}</span
            >
            <div class="min-w-0 flex-1 py-0.5">
              <RouterLink
                :to="contestProblemLocation(contestId, entry.problemId)"
                class="wrap-break-word text-brand-primary underline underline-offset-2 hover:text-brand-secondary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
              >
                {{ entry.problem?.title || `問題 ${entry.label}` }}
              </RouterLink>
              <p v-if="entry.error" class="mt-2 text-text-secondary">
                問題を読み込めませんでした。
              </p>
            </div>
            <MonochromeButton v-if="entry.error" small @click="refresh">再試行</MonochromeButton>
          </li>
        </ol>
      </template>
    </QueryState>
  </section>
</template>
