<script setup lang="ts">
import type { User } from '@/api/generated'
import NumberedPagination from '@/components/NumberedPagination.vue'
import ProblemFilters from '@/components/ProblemFilters.vue'
import ProblemsList from '@/components/ProblemsList.vue'
import QueryState from '@/components/QueryState.vue'
import { useProblemsQuery } from '@/composables/problems/useProblemsQuery'
import { useListRoute } from '@/router/listConditions'
const props = defineProps<{ userId: string; user: User | null }>()
const { conditions, updateConditions } = useListRoute()
const { view, refresh } = useProblemsQuery(
  () => ({ kind: 'user', userId: props.userId }),
  conditions
)
</script>

<template>
  <section class="flex flex-col gap-4">
    <ProblemFilters
      :difficulty-begin="conditions.difficultyBegin"
      :difficulty-end="conditions.difficultyEnd"
      @filter="updateConditions($event)"
    >
      <span class="mr-1">{{ props.user?.name }}</span
      ><span class="grow">の問題</span>
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
