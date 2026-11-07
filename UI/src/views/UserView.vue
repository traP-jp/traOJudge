<script setup lang="ts">
import { useRoute } from 'vue-router'
import SideMenuUserPage from '@/components/Navigations/SideMenu/SideMenuUserPage.vue'
import { ref, watch } from 'vue'
import { UsersApi, type Profile, ResponseError } from '@/api/generated'

const route = useRoute()

if (typeof route.params.id !== 'string') throw new Error('Invalid route')
const userId = ref<string>('')
const user = ref<Profile | null>(null)

const loadUser = async () => {
  try {
    user.value = await new UsersApi().getUserProfile({ userId: userId.value })
  } catch (error) {
    if (error instanceof ResponseError) {
      switch (error.response.status) {
        case 404:
          alert('ユーザーが見つかりませんでした。')
          break
        default:
          alert('エラーが発生しました。')
          break
      }
    } else {
      console.error('API Error:', error)
      alert('不明なエラーが発生しました。')
    }
  }
}

watch(
  () => route.params.id,
  (id) => {
    userId.value = `${id}`
    loadUser()
  },
  { immediate: true }
)
</script>

<template>
  <div class="flex gap-8 px-container-x">
    <nav class="sticky top-14 h-header-offset">
      <SideMenuUserPage :user-id="userId" :username="user?.displayName ?? ''" />
    </nav>
    <main class="flex-auto py-6">
      <RouterView :user-id="userId" :user="user" />
    </main>
  </div>
</template>
