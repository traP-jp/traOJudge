import { computed, type MaybeRefOrGetter, toValue } from 'vue'
import { useRouter } from 'vue-router'

import { SubmissionsApi } from '@/api/generated'
import { useSessionQuery } from '@/composables/useSessionQuery'
import { pageSize } from '@/router/listConditions'
import { problemLocation, userLocation } from '@/router/locations'

export type SubmissionListTarget =
  | { kind: 'problem'; problemId: string }
  | { kind: 'user'; userId: string }
export const submissionsKey = (target: SubmissionListTarget) => ['submissions', target] as const

export const useSubmissionsQuery = (
  target: MaybeRefOrGetter<SubmissionListTarget>,
  page: MaybeRefOrGetter<number>
) => {
  const router = useRouter()
  const result = useSessionQuery({
    key: () => [...submissionsKey(toValue(target)), { page: toValue(page) }],
    enabled: () => {
      const scope = toValue(target)
      return !!(scope.kind === 'problem' ? scope.problemId : scope.userId)
    },
    fetch: (signal) => {
      const scope = toValue(target)
      return new SubmissionsApi().getSubmissions(
        {
          problemIdInQuery: scope.kind === 'problem' ? scope.problemId : undefined,
          userId: scope.kind === 'user' ? scope.userId : undefined,
          orderBy: 'submittedAtDesc',
          limit: pageSize,
          offset: (toValue(page) - 1) * pageSize
        },
        { signal }
      )
    }
  })
  const view = computed(() => {
    const current = result.view.value
    if (current.status !== 'success') return current
    return {
      ...current,
      data: {
        rows: (current.data.submissions ?? []).map((submission) => ({
          ...submission,
          problemHref: router.resolve(problemLocation(submission.problemId)).href,
          userHref: router.resolve(userLocation(submission.userId)).href
        })),
        totalPages: Math.max(1, Math.ceil((current.data.total ?? 0) / pageSize))
      }
    }
  })
  return { view, refresh: result.refresh }
}
