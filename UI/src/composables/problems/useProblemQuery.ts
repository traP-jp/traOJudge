import { type MaybeRefOrGetter, toValue } from 'vue'

import { ProblemsApi } from '@/api/generated'
import { useSessionQuery } from '@/composables/useSessionQuery'

export const useProblemQuery = (problemId: MaybeRefOrGetter<string>) =>
  useSessionQuery({
    key: () => ['problem', toValue(problemId)],
    enabled: () => !!toValue(problemId),
    fetch: (signal) => new ProblemsApi().getProblem({ problemId: toValue(problemId) }, { signal })
  })
