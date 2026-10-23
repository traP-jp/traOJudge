<script setup lang="ts">
import NumberedPagination from '@/components/controls/pagination/NumberedPagination.vue'
import QueryState from '@/components/QueryState.vue'
import SubmissionsFilter from '@/components/submissions/SubmissionsFilter.vue'
import SubmissionsTable from '@/components/submissions/SubmissionsTable.vue'
import { useSubmissionsQuery } from '@/composables/submissions/useSubmissionsQuery'
import { useListRoute } from '@/router/listConditions'
import { useRouteId } from '@/router/locations'
const problemId = useRouteId()
const { conditions, updateConditions } = useListRoute()
const { view, refresh } = useSubmissionsQuery(
  () => ({ kind: 'problem', problemId: problemId.value }),
  () => conditions.value.page
)
</script>

<template>
  <div class="flex w-full flex-col gap-6 py-6">
    <SubmissionsFilter><template #title>提出一覧</template></SubmissionsFilter>
    <QueryState :view="view" @retry="refresh">
      <template #default="{ data }">
        <SubmissionsTable :rows="data.rows" :columns="['userName']" />
        <NumberedPagination
          :page="conditions.page"
          :total-pages="data.totalPages"
          @change="updateConditions({ page: $event })"
          @normalize="updateConditions({ page: $event }, true)"
        />
      </template>
    </QueryState>
  </div>
</template>
