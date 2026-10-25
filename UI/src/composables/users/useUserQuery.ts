import { type MaybeRefOrGetter, toValue } from 'vue'

import { UsersApi } from '@/api/generated'
import { useSessionQuery } from '@/composables/useSessionQuery'

export const useUserQuery = (userId: MaybeRefOrGetter<string>) =>
  useSessionQuery({
    key: () => ['user', toValue(userId)],
    enabled: () => !!toValue(userId),
    fetch: (signal) => new UsersApi().getUser({ userId: toValue(userId) }, { signal })
  })
