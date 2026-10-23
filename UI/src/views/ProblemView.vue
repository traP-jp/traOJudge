<script setup lang="ts">
import { computed } from 'vue'

import SideMenuProblem from '@/components/Navigations/SideMenu/SideMenuProblem.vue'
import QueryState from '@/components/QueryState.vue'
import { useProblemQuery } from '@/composables/problems/useProblemQuery'
import { useRouteId } from '@/router/locations'
import { useUserStore } from '@/stores/user'

const userStore = useUserStore()
const problemId = useRouteId()
const { view, refresh } = useProblemQuery(problemId)
const isAuthor = computed(
  () => view.value.status === 'success' && userStore.userId === view.value.data.authorId
)
</script>

<template>
  <div class="flex items-start gap-8 px-container-x">
    <SideMenuProblem
      :problem-id="problemId"
      :problem-name="view.status === 'success' ? view.data.title : ''"
      :is-editorial-available="true"
      :is-admin="isAuthor"
    />
    <div class="min-w-0 flex-1">
      <QueryState :view="view" @retry="refresh">
        <RouterView />
      </QueryState>
    </div>
  </div>
</template>
