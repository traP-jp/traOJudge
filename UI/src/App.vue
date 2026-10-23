<script setup lang="ts">
import { RouterView, useRoute, useRouter } from 'vue-router'

import MonochromeButton from '@/components/Controls/MonochromeButton.vue'
import BottomNavbar from '@/components/Navigations/BottomNavbar.vue'
import TopNavbar from '@/components/Navigations/TopNavbar.vue'
import { useUserStore } from '@/stores/user'

const userStore = useUserStore()
const router = useRouter()
const route = useRoute()
const retry = async () => {
  await userStore.initialize()
  await router.replace({ path: route.path, query: route.query, hash: route.hash, force: true })
}
</script>

<template>
  <TopNavbar
    :is-logged-in="userStore.isAuthenticated"
    :user-id="userStore.userId"
    :username="userStore.username"
  />
  <main>
    <div
      v-if="!userStore.isInitialized && (route.meta.requiresAuth || userStore.error)"
      class="flex flex-col items-center gap-4 py-16"
      role="status"
    >
      <p>{{ userStore.error ?? '読み込み中...' }}</p>
      <MonochromeButton v-if="userStore.error" @click="retry">再試行</MonochromeButton>
    </div>
    <RouterView
      v-if="!route.meta.requiresAuth || (userStore.isInitialized && userStore.isAuthenticated)"
    />
  </main>
  <BottomNavbar />
</template>
