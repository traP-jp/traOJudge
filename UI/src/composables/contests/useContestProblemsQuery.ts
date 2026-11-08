import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import { apiClient, type Contest, requireData } from '@/api/client'
import { ApiError } from '@/api/errors'
import { useContestPhase } from '@/composables/contests/useContestPhase'
import { useApiQuery } from '@/composables/useApiQuery'
import { useUserStore } from '@/stores/user'
import { hasContestRole } from '@/utils/contestAccess'

export const useContestProblemsQuery = (contest: MaybeRefOrGetter<Contest | undefined>) => {
  const session = useUserStore()
  const { phase } = useContestPhase(contest)
  const canRead = computed(
    () =>
      phase.value !== 'upcoming' ||
      hasContestRole(session.user, toValue(contest)?.id ?? '', 'contest-setter')
  )
  return useApiQuery({
    keyPrefix: 'contest',
    key: () => ['problems', toValue(contest)?.id, canRead.value],
    enabled: () => !!toValue(contest),
    fetch: async (signal) => {
      const value = toValue(contest)
      if (!value) throw new Error('Missing contest')
      if (!canRead.value) return { awaitingStart: true, problems: [] }
      const viewer = { signal, credentials: session.isAuthenticated ? 'include' : 'omit' } as const
      const { data } = await apiClient.GET('/contests/{contestId}/problems', {
        ...viewer,
        params: { path: { contestId: value.id } }
      })
      const problems = await Promise.all(
        [...requireData(data).items]
          .sort((a, b) => a.order - b.order || a.label.localeCompare(b.label))
          .map(async (entry) => {
            try {
              const { data } = await apiClient.GET('/problems/{problemId}', {
                ...viewer,
                params: { path: { problemId: entry.problemId } }
              })
              const problem = requireData(data)
              if (problem.id !== entry.problemId) throw new Error('Unexpected problem')
              return { ...entry, problem, error: false }
            } catch (error) {
              signal.throwIfAborted()
              if (error instanceof ApiError && error.response.status === 401) throw error
              return { ...entry, problem: undefined, error: true }
            }
          })
      )
      return { awaitingStart: false, problems }
    }
  })
}
