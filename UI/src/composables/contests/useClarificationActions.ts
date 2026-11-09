import { type MaybeRefOrGetter, onScopeDispose, ref, toValue, watch } from 'vue'

import { apiClient, type Clarification, requireData } from '@/api/client'
import { ApiError } from '@/api/errors'
import { queryClient } from '@/api/queryClient'
import { useUserStore } from '@/stores/user'
import { hasContestRole } from '@/utils/contestAccess'

export const useClarificationActions = (contestId: MaybeRefOrGetter<string>) => {
  const session = useUserStore()
  const isSaving = ref(false)
  let controller: AbortController | undefined
  let pending: { body: string; key: string } | undefined
  const cancel = () => {
    controller?.abort()
    isSaving.value = false
    pending = undefined
  }
  watch(() => [session.epoch, session.userId, toValue(contestId)], cancel, { flush: 'sync' })
  onScopeDispose(cancel)
  const run = async (
    role: 'contestant' | 'contest-setter',
    operation: (id: string, signal: AbortSignal) => Promise<Clarification>
  ) => {
    if (isSaving.value) return
    const id = toValue(contestId)
    if (!session.isAuthenticated || !hasContestRole(session.user, id, role))
      throw new Error('この操作を行う権限がありません。')
    controller = new AbortController()
    const { signal } = controller
    isSaving.value = true
    try {
      const saved = await operation(id, signal)
      signal.throwIfAborted()
      void queryClient.invalidateQueries({ queryKey: ['contest'] })
      return saved
    } catch (error) {
      if (signal.aborted) return
      if (error instanceof ApiError && error.response.status === 401) session.reset()
      if (error instanceof ApiError && [409, 412].includes(error.response.status)) {
        void queryClient.invalidateQueries({ queryKey: ['contest'] })
        throw new Error('質問が更新されています。最新の質問と回答を確認してください。', {
          cause: error
        })
      }
      throw error
    } finally {
      if (!signal.aborted) isSaving.value = false
    }
  }
  const validate = (body: string) => {
    if (!body.trim() || Array.from(body).length > 10000)
      throw new Error('本文は1〜10000文字で入力してください。')
  }
  const create = (body: string, problemId?: string) =>
    run('contestant', async (id, signal) => {
      validate(body)
      const input = { body, ...(problemId ? { problemId } : {}) }
      const serialized = JSON.stringify(input)
      if (pending?.body !== serialized) pending = { body: serialized, key: crypto.randomUUID() }
      const { data } = await apiClient.POST('/contests/{contestId}/clarifications', {
        params: {
          path: { contestId: id },
          header: { Origin: window.location.origin, 'Idempotency-Key': pending.key }
        },
        body: input,
        signal
      })
      signal.throwIfAborted()
      pending = undefined
      return requireData(data)
    })
  const answer = (clarificationId: string, body: string, isPublic: boolean) =>
    run('contest-setter', async (id, signal) => {
      validate(body)
      const path = { contestId: id, clarificationId }
      const current = await apiClient.GET(
        '/contests/{contestId}/clarifications/{clarificationId}',
        { params: { path }, signal }
      )
      signal.throwIfAborted()
      const question = requireData(current.data)
      if (question.id !== clarificationId) throw new Error('回答を保存できませんでした。')
      if (question.answer === body && question.isPublic === isPublic) return question
      if (question.answer !== null) {
        void queryClient.invalidateQueries({ queryKey: ['contest'] })
        throw new Error('この質問には既に回答があります。')
      }
      const etag = current.response.headers.get('ETag')
      if (!etag) throw new Error('回答を保存できませんでした。')
      const { data } = await apiClient.PUT(
        '/contests/{contestId}/clarifications/{clarificationId}/answer',
        {
          params: { path, header: { Origin: window.location.origin, 'If-Match': etag } },
          body: { body, isPublic },
          signal
        }
      )
      return requireData(data)
    })
  return { create, answer, isSaving }
}
