<script setup lang="ts">
import type { Profile } from '@/api/client'
import NumberedPagination from '@/components/controls/pagination/NumberedPagination.vue'
import QueryState from '@/components/QueryState.vue'
import SubmissionsFilter from '@/components/submissions/SubmissionsFilter.vue'
import SubmissionsTable from '@/components/submissions/SubmissionsTable.vue'
import { useSubmissionsQuery } from '@/composables/submissions/useSubmissionsQuery'
import { useListRoute } from '@/router/listConditions'
const props = defineProps<{ userId: string; user: Profile | null }>()
const { conditions, updateConditions } = useListRoute()
const { view, refresh } = useSubmissionsQuery(
  () => ({ kind: 'user', userId: props.userId }),
  () => conditions.value.page
)
</script>

<template>
  <div class="flex w-full flex-col gap-6 py-6">
    <SubmissionsFilter
      ><template #title
        ><span>{{ props.user?.displayName }}</span
        ><span>の提出</span></template
      ></SubmissionsFilter
    >
    <QueryState :view="view" @retry="refresh">
      <template #default="{ data }">
        <SubmissionsTable :rows="data.rows" :columns="['title']" />
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
