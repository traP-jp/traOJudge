import { type MaybeRefOrGetter, toValue, watch } from 'vue'

import { apiClient, type JudgeRun, requireData } from '@/api/client'
import { useApiQuery } from '@/composables/useApiQuery'
import { useUserStore } from '@/stores/user'
import { isJudgeRunFinished, latestJudgeRun } from '@/utils/judgeRun'

export const useJudgeRunQuery = (
  target: MaybeRefOrGetter<{ submissionId: string; judgeRunId: string }>,
  enabled: MaybeRefOrGetter<boolean>
) => {
  const session = useUserStore()
  let latest: JudgeRun | undefined
  watch(
    () => [session.epoch, session.userId, toValue(target).submissionId, toValue(target).judgeRunId],
    () => {
      latest = undefined
    },
    { flush: 'sync' }
  )
  return useApiQuery<JudgeRun>({
    keyPrefix: 'contest',
    key: () => ['judge-run', toValue(target).submissionId, toValue(target).judgeRunId],
    enabled,
    requiresAuth: true,
    refetchInterval: (data) => (isJudgeRunFinished(data) ? false : 2000),
    fetch: async (signal): Promise<JudgeRun> => {
      const path = { ...toValue(target) }
      const { data } = await apiClient.GET('/submissions/{submissionId}/judge-runs/{judgeRunId}', {
        params: { path },
        signal
      })
      signal.throwIfAborted()
      const incoming = requireData(data)
      if (incoming.id !== path.judgeRunId || incoming.submissionId !== path.submissionId)
        throw new Error('採点結果の対象が一致しません。')
      latest = latestJudgeRun(latest, incoming)
      return latest
    }
  })
}
