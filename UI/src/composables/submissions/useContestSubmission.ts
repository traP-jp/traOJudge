import { type MaybeRefOrGetter, onScopeDispose, ref, toValue, watch } from 'vue'

import { apiClient, requireData } from '@/api/client'
import { ApiError } from '@/api/errors'
import type { components } from '@/api/generated/schema'
import { queryClient } from '@/api/queryClient'
import { useUserStore } from '@/stores/user'
import { hasContestRole } from '@/utils/contestAccess'
import { sourceFileManifest } from '@/utils/sourceFileManifest'

type Input = { languageId: string; filename: string; source: string }
type Target = { contestId: string; problemId: string }
type Attempt = {
  key: string
  uploadKey: string
  submissionKey: string
  upload?: components['schemas']['SourceUpload']
  uploaded: boolean
}

export const useContestSubmission = (target: MaybeRefOrGetter<Target>) => {
  const session = useUserStore()
  const isSubmitting = ref(false)
  let controller: AbortController | undefined
  let attempt: Attempt | undefined
  const reset = () => {
    controller?.abort()
    attempt = undefined
    isSubmitting.value = false
  }
  watch(
    () => [session.epoch, session.userId, toValue(target).contestId, toValue(target).problemId],
    reset,
    { flush: 'sync' }
  )
  onScopeDispose(reset)

  const uploadSource = async (
    upload: components['schemas']['SourceUpload'],
    content: Uint8Array<ArrayBuffer>,
    path: string,
    signal: AbortSignal
  ) => {
    const destination = upload.targets[0]
    if (
      upload.targets.length !== 1 ||
      !destination ||
      destination.path !== path ||
      destination.method !== 'PUT'
    )
      throw new Error('提出の準備ができませんでした。')
    const expiresAt = Math.min(Date.parse(upload.expiresAt), Date.parse(destination.expiresAt))
    if (!Number.isFinite(expiresAt) || expiresAt <= Date.now()) {
      attempt = undefined
      throw new Error('提出できませんでした。もう一度提出してください。')
    }
    const url = new URL(destination.url)
    if (!['https:', 'http:'].includes(url.protocol))
      throw new Error('提出の準備ができませんでした。')
    const response = await fetch(destination.url, {
      method: destination.method,
      headers: destination.headers,
      body: content,
      credentials: 'omit',
      signal
    })
    signal.throwIfAborted()
    if (!response.ok) throw new Error('コードを送信できませんでした。もう一度提出してください。')
  }

  const submit = async (input: Input) => {
    if (isSubmitting.value) return
    const { contestId, problemId } = toValue(target)
    if (!session.isAuthenticated || !hasContestRole(session.user, contestId, 'contestant'))
      throw new Error('このコンテストへの参加が必要です。')
    if (!problemId || !input.languageId || !input.source.trim())
      throw new Error('問題・言語・ソースコードを入力してください。')
    const key = JSON.stringify({ contestId, problemId, ...input })
    if (attempt?.key !== key)
      attempt = {
        key,
        uploadKey: crypto.randomUUID(),
        submissionKey: crypto.randomUUID(),
        uploaded: false
      }
    const current = attempt
    controller = new AbortController()
    const { signal } = controller
    isSubmitting.value = true
    try {
      const content = new TextEncoder().encode(input.source)
      const file = await sourceFileManifest(input.filename, content)
      signal.throwIfAborted()
      if (!current.upload) {
        const { data } = await apiClient.POST('/submissions/uploads', {
          params: {
            header: { Origin: window.location.origin, 'Idempotency-Key': current.uploadKey }
          },
          body: {
            problemId,
            languageId: input.languageId,
            context: { kind: 'contest', contestId },
            files: [file]
          },
          signal
        })
        signal.throwIfAborted()
        current.upload = requireData(data)
      }
      if (!current.uploaded) await uploadSource(current.upload, content, file.path, signal)
      current.uploaded = true
      // 確定応答を受け取れなかった場合も、同じキーとuploadIdで再送する。
      const { data } = await apiClient.POST('/submissions', {
        params: {
          header: { Origin: window.location.origin, 'Idempotency-Key': current.submissionKey }
        },
        body: { uploadId: current.upload.uploadId },
        signal
      })
      signal.throwIfAborted()
      const accepted = requireData(data)
      attempt = undefined
      void queryClient.invalidateQueries({ queryKey: ['contest'] })
      return accepted
    } catch (error) {
      if (signal.aborted) return
      if (error instanceof ApiError && error.response.status === 401) session.reset()
      throw error
    } finally {
      if (!signal.aborted) isSubmitting.value = false
    }
  }
  return { submit, isSubmitting }
}
