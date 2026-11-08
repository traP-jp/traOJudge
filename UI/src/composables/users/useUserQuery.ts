import { type MaybeRefOrGetter, toValue } from 'vue'

import { apiClient, requireData } from '@/api/client'
import { useApiQuery } from '@/composables/useApiQuery'
import { useUserStore } from '@/stores/user'

export const useUserQuery = (userId: MaybeRefOrGetter<string>) => {
  const session = useUserStore()
  return useApiQuery({
    keyPrefix: 'user',
    key: () => [toValue(userId)],
    enabled: () => !!toValue(userId),
    fetch: async (signal) => {
      const { data } = await apiClient.GET('/users/{userId}', {
        params: { path: { userId: toValue(userId) } },
        credentials: session.isAuthenticated ? 'include' : 'omit',
        signal
      })
      return requireData(data)
    }
  })
}
