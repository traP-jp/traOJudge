<script setup lang="ts">
import { computed } from 'vue'

import ContestHeader from '@/components/contests/ContestHeader.vue'
import ContestSubmitForm from '@/components/contests/ContestSubmitForm.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import { useContestQuery } from '@/composables/contests/useContestQuery'
import { useContestId } from '@/router/locations'

const contestQuery = useContestQuery(useContestId())
const contest = computed(() =>
  contestQuery.view.value.status === 'success' ? contestQuery.view.value.data : undefined
)
const problems = useContestProblemsQuery(contest)
</script>

<template>
  <main class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <QueryState :view="contestQuery.view.value" @retry="contestQuery.refresh">
      <template #default="{ data }">
        <ContestHeader :contest="data" />
        <section class="py-8">
          <h2 class="fontstyle-ui-subtitle">提出</h2>
          <QueryState :view="problems.view.value" @retry="problems.refresh">
            <template #default="{ data: page }">
              <p v-if="page.awaitingStart" class="py-6 text-text-secondary">提出受付前です。</p>
              <p v-else-if="page.problems.length === 0" class="py-6 text-text-secondary">
                提出できる問題がありません。
              </p>
              <ContestSubmitForm
                v-else
                :key="data.id"
                :contest="data"
                :problems="
                  page.problems.map((entry) => ({
                    id: entry.problemId,
                    name: `${entry.label} — ${entry.problem?.title || '問題'}`
                  }))
                "
              />
            </template>
          </QueryState>
        </section>
      </template>
    </QueryState>
  </main>
</template>
