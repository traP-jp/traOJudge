<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import type { Contest } from '@/api/client'
import { useContestPhase } from '@/composables/contests/useContestPhase'
import { contestLocation } from '@/router/locations'
import { formatContestDuration } from '@/utils/contest'

const props = defineProps<{ contestId: string; contest: Contest | undefined }>()
const route = useRoute()
const { now, phase } = useContestPhase(() => props.contest)

const tabs = [
  { label: 'トップ', name: 'contest-top', tab: 'top' },
  { label: '問題', name: 'contest-problems', tab: 'problems' },
  { label: '提出', name: 'contest-submit', tab: 'submit' },
  { label: '提出一覧', name: 'contest-submissions', tab: 'submissions' },
  { label: '質問', name: 'contest-clarifications', tab: 'clarifications' },
  { label: '解説', name: 'contest-editorials', tab: 'editorials' }
] as const

const isActive = (tab: (typeof tabs)[number]) =>
  route.name === tab.name ||
  (tab.tab === 'problems' && route.name === 'contest-problem') ||
  (tab.tab === 'editorials' && route.name === 'contest-editorial')

const phaseLabel = computed(() => {
  if (phase.value === 'upcoming') return '開催予定'
  if (phase.value === 'running') return '開催中'
  return '終了'
})
const remainingTime = computed(() => {
  if (!props.contest) return ''
  if (phase.value === 'finished') return 'コンテストは終了しました'
  const target = phase.value === 'upcoming' ? props.contest.startsAt : props.contest.endsAt
  const label = phase.value === 'upcoming' ? '開始まで' : '終了まで'
  return `${label} ${formatContestDuration(Date.parse(target) - now.value)}`
})
</script>

<template>
  <header class="pb-6" :aria-busy="!contest">
    <span v-if="!contest" role="status" class="sr-only">コンテストを読み込み中...</span>
    <div class="mb-4 flex flex-wrap items-center gap-3 text-sm">
      <span
        v-if="contest"
        class="rounded-sm px-3 py-1 font-medium"
        :class="
          phase === 'running'
            ? 'bg-brand-primary text-white'
            : 'bg-background-secondary text-text-secondary'
        "
        >{{ phaseLabel }}</span
      >
      <span v-else aria-hidden="true" class="rounded-sm bg-background-secondary px-3 py-1">
        <span class="invisible">開催中</span>
      </span>
      <span v-if="contest?.visibility === 'private'" class="text-text-secondary">非公開</span>
    </div>
    <h1
      :aria-label="contest ? undefined : 'コンテスト'"
      class="text-2xl leading-snug font-semibold wrap-break-word text-text-primary sm:text-3xl"
    >
      <template v-if="contest">{{ contest.title }}</template>
      <span
        v-else
        aria-hidden="true"
        class="block w-72 max-w-full rounded-sm bg-background-secondary"
        >&nbsp;</span
      >
    </h1>
    <p class="mt-4 text-sm text-text-secondary tabular-nums">
      <template v-if="contest">{{ remainingTime }}</template>
      <span
        v-else
        aria-hidden="true"
        class="block w-48 max-w-full rounded-sm bg-background-secondary"
        >&nbsp;</span
      >
    </p>
  </header>

  <nav aria-label="コンテストのページ" class="border-b border-border-primary">
    <div class="flex flex-wrap gap-1">
      <RouterLink
        v-for="tab in tabs"
        :key="tab.name"
        :to="contestLocation(contestId, tab.tab)"
        :aria-current="isActive(tab) ? 'page' : undefined"
        class="-mb-px inline-flex min-h-12 items-center border-b-2 px-5 text-sm focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
        :class="
          isActive(tab)
            ? 'border-brand-primary font-semibold text-brand-primary'
            : 'border-transparent text-text-secondary hover:border-border-secondary hover:text-text-primary'
        "
        >{{ tab.label }}</RouterLink
      >
    </div>
  </nav>
</template>
