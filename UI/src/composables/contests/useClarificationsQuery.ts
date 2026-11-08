import { type MaybeRefOrGetter, toValue } from 'vue'

import { apiClient, requireData } from '@/api/client'
import { useApiQuery } from '@/composables/useApiQuery'

export const useClarificationsQuery = (
  contestId: MaybeRefOrGetter<string>,
  cursor: MaybeRefOrGetter<string | undefined>
) => {
  return useApiQuery({
    keyPrefix: 'contest',
    requiresAuth: true,
    key: () => ['clarifications', toValue(contestId), toValue(cursor)],
    enabled: () => !!toValue(contestId),
    fetch: async (signal) => {
      const { data } = await apiClient.GET('/contests/{contestId}/clarifications', {
        params: {
          path: { contestId: toValue(contestId) },
          query: { limit: 20, cursor: toValue(cursor) }
        },
        signal
      })
      return requireData(data)
    }
  })
}
