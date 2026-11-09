import { type MaybeRefOrGetter, toValue } from 'vue'

import { apiClient, requireData } from '@/api/client'
import { useApiQuery } from '@/composables/useApiQuery'
import { useUserStore } from '@/stores/user'

export type ContestSubmissionScope = 'all' | 'mine' | 'team'
export const useContestSubmissionsQuery = (
  conditions: MaybeRefOrGetter<{
    contestId: string
    scope: ContestSubmissionScope
    teamId: string
    cursor?: string
    enabled: boolean
  }>
) => {
  const session = useUserStore()
  return useApiQuery({
    keyPrefix: 'contest',
    key: () => {
      const value = toValue(conditions)
      return ['submissions', value.contestId, value.scope, value.teamId, value.cursor]
    },
    enabled: () => toValue(conditions).enabled,
    fetch: async (signal) => {
      const { contestId, scope, teamId, cursor } = toValue(conditions)
      const query = { limit: 20, cursor }
      const credentials = session.isAuthenticated ? 'include' : 'omit'
      const response =
        scope === 'mine'
          ? await apiClient.GET('/me/submissions', { params: { query }, signal })
          : scope === 'team'
            ? await apiClient.GET('/contests/{contestId}/teams/{teamId}/submissions', {
                params: { path: { contestId, teamId }, query },
                signal
              })
            : await apiClient.GET('/contests/{contestId}/submissions', {
                params: { path: { contestId }, query },
                credentials,
                signal
              })
      const page = requireData(response.data)
      return {
        ...page,
        items: page.items.filter(
          (item) => item.context.kind === 'contest' && item.context.contestId === contestId
        )
      }
    }
  })
}
