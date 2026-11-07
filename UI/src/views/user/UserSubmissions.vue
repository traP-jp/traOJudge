<script setup lang="ts">
import type { Profile } from '@/api/generated'
import SubmissionsTable from '@/components/SubmissionsTable.vue'
import { useUserStore } from '@/stores/user'
defineProps<{ userId: string; user: Profile | null }>()
const userStore = useUserStore()
</script>
<template>
  <section class="flex flex-col gap-4 py-6">
    <h2 class="fontstyle-ui-subtitle">{{ user?.displayName }}の提出</h2>
    <SubmissionsTable v-if="userId === userStore.userId" :key="userId" />
    <p v-else>この利用者の提出一覧は公開されていません．</p>
  </section>
</template>
