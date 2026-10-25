<script setup lang="ts">
import NumberedPagination from '@/components/controls/pagination/NumberedPagination.vue'
import ProblemFilters from '@/components/problems/ProblemFilters.vue'
import ProblemsList from '@/components/problems/ProblemsList.vue'
import QueryState from '@/components/QueryState.vue'
import { useProblemsQuery } from '@/composables/problems/useProblemsQuery'
import { useListRoute } from '@/router/listConditions'
const { conditions, updateConditions } = useListRoute()
const { view, refresh } = useProblemsQuery({ kind: 'all' }, conditions)
</script>

<template>
  <section class="flex flex-col gap-4 px-container-x py-6">
    <ProblemFilters
      :difficulty-begin="conditions.difficultyBegin"
      :difficulty-end="conditions.difficultyEnd"
      @filter="updateConditions($event)"
    >
      <span class="grow">問題一覧</span>
    </ProblemFilters>
    <QueryState :view="view" @retry="refresh">
      <template #default="{ data }">
        <ProblemsList :rows="data.problems" />
        <NumberedPagination
          :page="conditions.page"
          :total-pages="data.totalPages"
          @change="updateConditions({ page: $event })"
          @normalize="updateConditions({ page: $event }, true)"
        />
      </template>
    </QueryState>
  </section>
</template>
