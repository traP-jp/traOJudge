<script setup lang="ts">
import { computed } from 'vue'

import ContestHeader from '@/components/contests/ContestHeader.vue'
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestQuery } from '@/composables/contests/useContestQuery'
import { useEditorialsQuery } from '@/composables/contests/useEditorialsQuery'
import { contestEditorialLocation, useContestId } from '@/router/locations'

const contestQuery = useContestQuery(useContestId())
const currentContest = computed(() =>
  contestQuery.view.value.status === 'success' ? contestQuery.view.value.data : undefined
)
const { view, refresh } = useEditorialsQuery(currentContest)
</script>
<template>
  <main class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <QueryState :view="contestQuery.view.value" @retry="contestQuery.refresh">
      <template #default="{ data: contest }">
        <ContestHeader :contest="contest" />
        <QueryState :view="view" @retry="refresh">
          <template #default="{ data: page }">
            <section aria-labelledby="contest-editorials-heading" class="max-w-5xl py-8">
              <h2 id="contest-editorials-heading" class="mb-2 fontstyle-ui-subtitle">解説一覧</h2>
              <p v-if="page.awaitingRelease" class="py-10 leading-7 text-text-secondary">
                解説はコンテスト終了後に公開されます．
              </p>
              <p v-else-if="page.problems.length === 0" class="py-10 leading-7 text-text-secondary">
                このコンテストには問題が登録されていません．
              </p>
              <ol v-else class="divide-y divide-border-primary">
                <li v-for="problem in page.problems" :key="problem.problemId" class="py-8">
                  <div class="mb-5 flex min-w-0 items-start gap-3">
                    <span
                      class="shrink-0 rounded-sm bg-background-secondary px-3 py-1 text-sm font-semibold text-text-primary"
                      >{{ problem.label }}</span
                    >
                    <h2 class="min-w-0 text-lg font-semibold wrap-break-word text-text-primary">
                      {{ problem.title || `問題 ${problem.label}` }}
                    </h2>
                  </div>
                  <div v-if="problem.error" role="alert" class="flex items-center gap-3">
                    <p class="leading-7 text-text-secondary">
                      この問題の解説を読み込めませんでした．
                    </p>
                    <MonochromeButton small @click="refresh">再試行</MonochromeButton>
                  </div>
                  <p
                    v-else-if="problem.editorials.length === 0"
                    class="leading-7 text-text-secondary"
                  >
                    解説はまだ掲載されていません．
                  </p>
                  <ul v-else class="space-y-3">
                    <li v-for="editorial in problem.editorials" :key="editorial.id">
                      <RouterLink
                        :to="contestEditorialLocation(contest.id, problem.problemId, editorial.id)"
                        class="wrap-break-word text-brand-primary underline underline-offset-2 hover:text-brand-secondary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
                        >{{ editorial.title }}</RouterLink
                      >
                    </li>
                  </ul>
                </li>
              </ol>
            </section>
          </template>
        </QueryState>
      </template>
    </QueryState>
  </main>
</template>
