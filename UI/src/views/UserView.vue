<script setup lang="ts">
import SideMenuUserPage from '@/components/Navigations/SideMenu/SideMenuUserPage.vue'
import QueryState from '@/components/QueryState.vue'
import { useUserQuery } from '@/composables/users/useUserQuery'
import { useRouteId } from '@/router/locations'
const userId = useRouteId()
const { view, refresh } = useUserQuery(userId)
</script>

<template>
  <div class="flex gap-8 px-container-x">
    <nav class="sticky top-14 h-header-offset">
      <SideMenuUserPage
        :user-id="userId"
        :username="(view.status === 'success' ? view.data.name : '') ?? ''"
        :icon-url="(view.status === 'success' ? view.data.iconUrl : '') ?? ''"
      />
    </nav>
    <main class="flex-auto py-6">
      <QueryState :view="view" @retry="refresh">
        <template #default="{ data }"><RouterView :user-id="userId" :user="data" /></template>
      </QueryState>
    </main>
  </div>
</template>
