<script setup lang="ts">
import { computed } from 'vue'

import ContestHeader from '@/components/contests/ContestHeader.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestQuery } from '@/composables/contests/useContestQuery'
import { useContestId } from '@/router/locations'
import { useUserStore } from '@/stores/user'

const contestId = useContestId()
const session = useUserStore()
const { view, refresh } = useContestQuery(contestId)
const contest = computed(() => (view.value.status === 'success' ? view.value.data : undefined))
</script>

<template>
  <main class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <ContestHeader v-if="view.status !== 'error'" :contest-id="contestId" :contest="contest" />
    <QueryState v-if="view.status !== 'pending'" :view="view" @retry="refresh" />
    <RouterView
      v-if="view.status !== 'error'"
      :key="`${session.epoch}:${contestId}`"
      :contest="contest"
    />
  </main>
</template>
