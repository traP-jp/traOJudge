import { QueryClient } from '@tanstack/vue-query'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 0, retry: false, refetchOnWindowFocus: false },
    mutations: { retry: false }
  }
})

export const sessionQueryKey = (epoch: number, key: readonly unknown[]) => ['legacy', epoch, ...key]
