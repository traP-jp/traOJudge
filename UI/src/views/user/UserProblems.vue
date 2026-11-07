<script setup lang="ts">
import type { Profile } from '@/api/generated'
import ProblemsList from '@/components/ProblemsList.vue'
defineProps<{ userId: string; user: Profile | null }>()
</script>
<template>
  <section class="flex flex-col gap-4">
    <ProblemsList :created-by-user-id="userId">{{ user?.displayName }}の問題</ProblemsList>
  </section>
</template>
