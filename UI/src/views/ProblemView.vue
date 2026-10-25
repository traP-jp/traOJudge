<script setup lang="ts">
import SideMenuProblem from '@/components/problems/SideMenuProblem.vue'
import QueryState from '@/components/QueryState.vue'
import { useMockAuthGuard } from '@/composables/problems/useMockAuthGuard'
import { useProblemQuery } from '@/composables/problems/useProblemQuery'
import { useRouteId } from '@/router/locations'

const { canEditProblem } = useMockAuthGuard()
const problemId = useRouteId()
const { view, refresh } = useProblemQuery(problemId)
</script>

<template>
  <div class="flex items-start gap-8 px-container-x">
    <SideMenuProblem
      :problem-id="problemId"
      :problem-name="view.status === 'success' ? view.data.title : ''"
      :is-editorial-available="true"
      :is-admin="canEditProblem"
    />
    <div class="min-w-0 flex-1">
      <QueryState :view="view" @retry="refresh">
        <RouterView />
      </QueryState>
    </div>
  </div>
</template>
