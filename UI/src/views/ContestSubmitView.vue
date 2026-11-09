<script setup lang="ts">
import { computed } from 'vue'

import type { Contest } from '@/api/client'
import ContestSubmitForm from '@/components/contests/ContestSubmitForm.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import { useContestId } from '@/router/locations'

const contestId = useContestId()
const props = defineProps<{ contest: Contest | undefined }>()
const contest = computed(() => props.contest)
const problems = useContestProblemsQuery(contest)
</script>

<template>
  <section class="py-8">
    <h2 class="fontstyle-ui-subtitle">提出</h2>
    <QueryState :view="problems.view.value" @retry="problems.refresh">
      <template #default="{ data: page }">
        <p v-if="page.awaitingStart" class="py-6 text-text-secondary">提出受付前です。</p>
        <p v-else-if="page.problems.length === 0" class="py-6 text-text-secondary">
          提出できる問題がありません。
        </p>
        <ContestSubmitForm
          v-else-if="contest"
          :key="contestId"
          :contest="contest"
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
