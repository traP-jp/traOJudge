import { type MaybeRefOrGetter, toValue } from 'vue'

import { apiClient, requireData } from '@/api/client'
import { useApiQuery } from '@/composables/useApiQuery'
import { useUserStore } from '@/stores/user'

export const useContestQuery = (id: MaybeRefOrGetter<string>) => {
  const session = useUserStore()
  return useApiQuery({
    keyPrefix: 'contest',
    key: () => ['detail', toValue(id)],
    enabled: () => !!toValue(id),
    fetch: async (signal) => {
      const { data } = await apiClient.GET('/contests/{contestId}', {
        params: { path: { contestId: toValue(id) } },
        credentials: session.isAuthenticated ? 'include' : 'omit',
        signal
      })
      const contest = requireData(data)
      if (
        !Number.isFinite(Date.parse(contest.startsAt)) ||
        !Number.isFinite(Date.parse(contest.endsAt)) ||
        Date.parse(contest.endsAt) <= Date.parse(contest.startsAt)
      ) {
        throw new Error('Invalid contest schedule')
      }
      return contest
    }
  })
}
