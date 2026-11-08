<script setup lang="ts">
import { computed } from 'vue'

import ContestHeader from '@/components/contests/ContestHeader.vue'
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import { useContestQuery } from '@/composables/contests/useContestQuery'
import { contestProblemLocation, useContestId } from '@/router/locations'

const contestQuery = useContestQuery(useContestId())
const contest = computed(() =>
  contestQuery.view.value.status === 'success' ? contestQuery.view.value.data : undefined
)
const { view, refresh } = useContestProblemsQuery(contest)
</script>

<template>
  <main class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <QueryState :view="contestQuery.view.value" @retry="contestQuery.refresh">
      <template #default="{ data }">
        <ContestHeader :contest="data" />
        <section class="py-8">
          <h2 class="mb-6 fontstyle-ui-subtitle">問題一覧</h2>
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
                  class="flex items-start gap-4 py-5"
                >
                  <span class="rounded-sm bg-background-secondary px-3 py-1 font-semibold">{{
                    entry.label
                  }}</span>
                  <div class="min-w-0 flex-1">
                    <RouterLink
                      :to="contestProblemLocation(data.id, entry.problemId)"
                      class="wrap-break-word text-brand-primary underline underline-offset-2"
                    >
                      {{ entry.problem?.title || `問題 ${entry.label}` }}
                    </RouterLink>
                    <p v-if="entry.error" class="mt-2 text-text-secondary">
                      問題を読み込めませんでした。
                    </p>
                  </div>
                  <MonochromeButton v-if="entry.error" small @click="refresh"
                    >再試行</MonochromeButton
                  >
                </li>
              </ol>
            </template>
          </QueryState>
        </section>
      </template>
    </QueryState>
  </main>
</template>
