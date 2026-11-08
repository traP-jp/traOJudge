import { apiClient, requireData } from '@/api/client'
import { useApiQuery } from '@/composables/useApiQuery'

export const useSubmissionLanguages = () =>
  useApiQuery({
    keyPrefix: 'contest',
    key: ['languages'],
    fetch: async (signal) => {
      const { data } = await apiClient.GET('/languages', { signal, credentials: 'omit' })
      return requireData(data).items
    }
  })
