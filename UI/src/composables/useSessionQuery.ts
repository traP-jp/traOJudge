import type { MaybeRefOrGetter } from 'vue'

import { useApiQuery } from '@/composables/useApiQuery'

export const useSessionQuery = <T>(options: {
  key: MaybeRefOrGetter<readonly unknown[]>
  fetch: (signal: AbortSignal) => Promise<T>
  enabled?: MaybeRefOrGetter<boolean>
}) => useApiQuery({ ...options, keyPrefix: 'legacy', requiresAuth: true })
