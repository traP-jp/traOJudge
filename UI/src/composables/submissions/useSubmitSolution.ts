import { ref } from 'vue'

import { type PostSubmissionRequest, SubmissionsApi } from '@/api/generated'
import { queryClient, sessionQueryKey } from '@/api/queryClient'
import { submissionsKey } from '@/composables/submissions/useSubmissionsQuery'
import { useUserStore } from '@/stores/user'

export const useSubmitSolution = () => {
  const session = useUserStore()
  const isSubmitting = ref(false)
  const submit = async (problemId: string, input: PostSubmissionRequest): Promise<boolean> => {
    if (isSubmitting.value || !session.isAuthenticated) return false
    const epoch = session.epoch
    const userId = session.userId
    const request = { problemId, postSubmissionRequest: { ...input } }
    isSubmitting.value = true
    try {
      await new SubmissionsApi().postSubmission(request)
      if (epoch !== session.epoch) return false
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: sessionQueryKey(epoch, submissionsKey({ kind: 'problem', problemId }))
        }),
        queryClient.invalidateQueries({
          queryKey: sessionQueryKey(epoch, submissionsKey({ kind: 'user', userId }))
        })
      ])
      return epoch === session.epoch
    } finally {
      isSubmitting.value = false
    }
  }
  return { submit, isSubmitting }
}
