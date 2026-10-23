import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import { ProblemsApi } from '@/api/generated'
import { useSessionQuery } from '@/composables/useSessionQuery'
import { type ListConditions, pageSize } from '@/router/listConditions'

export type ProblemListTarget = { kind: 'all' } | { kind: 'user'; userId: string }

export const useProblemsQuery = (
  target: MaybeRefOrGetter<ProblemListTarget>,
  conditions: MaybeRefOrGetter<ListConditions>
) => {
  const request = computed(() => {
    const scope = toValue(target)
    return {
      orderBy: 'createdAtDesc' as const,
      userId: scope.kind === 'user' ? scope.userId : undefined,
      limit: pageSize,
      offset: (toValue(conditions).page - 1) * pageSize
    }
  })
  const result = useSessionQuery({
    key: () => ['problems', request.value],
    enabled: () => toValue(target).kind === 'all' || !!request.value.userId,
    fetch: (signal) => new ProblemsApi().getProblems(request.value, { signal })
  })
  const view = computed(() => {
    const current = result.view.value
    if (current.status !== 'success') return current
    const { difficultyBegin, difficultyEnd } = toValue(conditions)
    const problems = current.data.problems ?? []
    return {
      ...current,
      data: {
        // 旧APIは難易度で絞り込めないため、取得したページ内で絞り込む。
        problems: problems.filter(
          (p) => difficultyBegin <= p.difficulty && p.difficulty <= difficultyEnd
        ),
        totalPages: Math.max(1, Math.ceil((current.data.total ?? problems.length) / pageSize))
      }
    }
  })
  return { view, refresh: result.refresh }
}
