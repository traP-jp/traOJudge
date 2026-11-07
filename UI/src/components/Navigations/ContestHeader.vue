<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import type { GetContest200Response as Contest } from '@/api/generated/contest/models/GetContest200Response'
import { formatContestDuration, getContestPhase } from '@/utils/contest'

const props = defineProps<{ contest: Contest }>()
const route = useRoute()
const now = ref(Date.now())
let clock: ReturnType<typeof setInterval> | undefined

const tabs = [
  { label: 'トップ', name: 'contest-top' },
  { label: '解説', name: 'contest-editorials' }
] as const

const phase = computed(() =>
  getContestPhase(props.contest.startsAt, props.contest.endsAt, now.value)
)
const phaseLabel = computed(() => {
  if (phase.value === 'upcoming') return '開催予定'
  if (phase.value === 'running') return '開催中'
  return '終了'
})
const remainingTime = computed(() => {
  if (phase.value === 'finished') return 'コンテストは終了しました'
  const target = phase.value === 'upcoming' ? props.contest.startsAt : props.contest.endsAt
  const label = phase.value === 'upcoming' ? '開始まで' : '終了まで'
  return `${label} ${formatContestDuration(target.getTime() - now.value)}`
})

onMounted(() => {
  clock = setInterval(() => {
    now.value = Date.now()
  }, 1000)
})
onUnmounted(() => clearInterval(clock))
</script>

<template>
  <header class="pb-6">
    <div class="mb-4 flex flex-wrap items-center gap-3 text-sm">
      <span
        class="rounded px-3 py-1 font-medium"
        :class="
          phase === 'running'
            ? 'bg-brand-primary text-white'
            : 'bg-background-secondary text-text-secondary'
        "
        >{{ phaseLabel }}</span
      >
      <span v-if="contest.visibility === 'private'" class="text-text-secondary">非公開</span>
    </div>
    <h1 class="break-words text-2xl font-semibold leading-snug text-text-primary sm:text-3xl">
      {{ contest.title }}
    </h1>
    <p class="mt-4 text-sm tabular-nums text-text-secondary">{{ remainingTime }}</p>
  </header>

  <nav aria-label="コンテストのページ" class="border-b border-border-primary">
    <div class="flex gap-1">
      <RouterLink
        v-for="tab in tabs"
        :key="tab.name"
        :to="{ name: tab.name, params: { contestId: contest.id } }"
        :aria-current="route.name === tab.name ? 'page' : undefined"
        class="-mb-px inline-flex min-h-12 items-center border-b-2 px-5 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
        :class="
          route.name === tab.name
            ? 'border-brand-primary font-semibold text-brand-primary'
            : 'border-transparent text-text-secondary hover:border-border-secondary hover:text-text-primary'
        "
        >{{ tab.label }}</RouterLink
      >
    </div>
  </nav>
</template>
