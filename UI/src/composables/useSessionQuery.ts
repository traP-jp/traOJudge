import { useQuery } from '@tanstack/vue-query'
import { computed, type MaybeRefOrGetter, toValue, watch } from 'vue'

import { ResponseError } from '@/api/generated/runtime'
import { sessionQueryKey } from '@/api/queryClient'
import { useUserStore } from '@/stores/user'
import type { QueryView } from '@/types/query'

const errorMessage = (error: Error): string => {
  if (!(error instanceof ResponseError)) return '読み込みに失敗しました。もう一度お試しください。'
  switch (error.response.status) {
    case 401:
      return 'ログインが必要です。再度ログインしてください。'
    case 403:
      return '閲覧する権限がありません。'
    case 404:
      return '対象が見つかりませんでした。'
    default:
      return '読み込みに失敗しました。もう一度お試しください。'
  }
}

export const useSessionQuery = <T>(options: {
  key: MaybeRefOrGetter<readonly unknown[]>
  fetch: (signal: AbortSignal) => Promise<T>
  enabled?: MaybeRefOrGetter<boolean>
}) => {
  const session = useUserStore()
  const enabled = computed(
    () =>
      session.isInitialized &&
      session.isAuthenticated &&
      !session.isLoading &&
      (options.enabled === undefined || toValue(options.enabled))
  )
  const query = useQuery<T, Error, T>({
    queryKey: computed(() => sessionQueryKey(session.epoch, toValue(options.key))),
    enabled,
    refetchOnMount: (query) => query.getObserversCount() <= 1,
    queryFn: ({ signal }) => options.fetch(signal)
  })

  watch(
    query.error,
    (error) => {
      if (error instanceof ResponseError && error.response.status === 401) session.reset()
    },
    { flush: 'sync' }
  )

  const view = computed<QueryView<T>>(() => {
    if (!enabled.value) return { status: 'pending' }
    const error = query.error.value
    const denied = error instanceof ResponseError && [401, 403, 404].includes(error.response.status)
    if (error && (denied || query.data.value === undefined)) {
      return { status: 'error', message: errorMessage(error) }
    }
    if (query.data.value === undefined) return { status: 'pending' }
    return {
      status: 'success',
      data: query.data.value,
      refreshing: query.isFetching.value,
      warning: error ? errorMessage(error) : undefined
    }
  })
  const refresh = async (): Promise<void> => {
    if (enabled.value) await query.refetch()
  }
  // 権限エラー後もキャッシュにはデータが残るため、表示用のviewだけを公開する。
  return { view, refresh }
}
