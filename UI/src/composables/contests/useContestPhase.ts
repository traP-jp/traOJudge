import { computed, type MaybeRefOrGetter, onUnmounted, ref, toValue } from 'vue'

import type { Contest } from '@/api/client'
import { getContestPhase } from '@/utils/contest'

export const useContestPhase = (contest: MaybeRefOrGetter<Contest | undefined>) => {
  const now = ref(Date.now())
  const clock = setInterval(() => {
    now.value = Date.now()
  }, 1000)
  onUnmounted(() => clearInterval(clock))
  const phase = computed(() => {
    const value = toValue(contest)
    return value ? getContestPhase(value.startsAt, value.endsAt, now.value) : undefined
  })
  return { now, phase }
}
