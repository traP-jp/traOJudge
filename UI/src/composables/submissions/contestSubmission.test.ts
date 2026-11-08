import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import { useContestSubmission } from '@/composables/submissions/useContestSubmission'
import { cleanupQueries, deferred, me, mountQuery, setupSession } from '@/test/helpers'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
const { webcrypto } = await vi.importActual<{ webcrypto: Crypto }>('node:crypto')
const fetcher = vi.fn<typeof fetch>()
const input = { languageId: 'cpp', filename: 'main.cpp', source: '// 日本語\nint main() {}' }
const accepted = {
  submissionId: 'submission-a',
  judgeRunId: 'run-a',
  problemVersionId: 'version-a',
  acceptedAt: '2026-01-01T00:00:00Z'
}
const upload = {
  uploadId: 'upload-a',
  expiresAt: '2099-01-01T00:00:00Z',
  targets: [
    {
      fileId: 'file-a',
      path: 'main.cpp',
      method: 'PUT',
      url: 'https://storage.example.test/source',
      headers: { 'Content-Type': 'application/octet-stream' },
      expiresAt: '2099-01-01T00:00:00Z'
    }
  ]
}
let context: ReturnType<typeof setupSession>
beforeEach(() => {
  context = setupSession()
  context.session.user = {
    ...me,
    roles: [
      {
        role: 'contestant',
        scope: { kind: 'contest', contestId: 'contest-a' },
        userId: me.id,
        grantedAt: '2026-01-01T00:00:00Z'
      }
    ]
  }
  fetcher.mockReset()
  vi.stubGlobal('fetch', fetcher)
  vi.stubGlobal('crypto', webcrypto)
})
afterEach(() => {
  cleanupQueries()
  vi.unstubAllGlobals()
})

it('uploads UTF-8 bytes without cookies and retries an uncertain acceptance with the same key', async () => {
  const requests: Request[] = []
  let attempts = 0
  fetcher.mockImplementation((input, init) => {
    const request = new Request(input, init)
    requests.push(request.clone())
    if (request.url.endsWith('/uploads')) return Promise.resolve(Response.json(upload))
    if (request.method === 'PUT') return Promise.resolve(new Response(null, { status: 200 }))
    attempts++
    if (attempts === 1) return Promise.reject(new TypeError('Connection lost after acceptance'))
    return Promise.resolve(Response.json(accepted, { status: 202 }))
  })
  const { result } = mountQuery(
    () => useContestSubmission({ contestId: 'contest-a', problemId: 'problem-a' }),
    context
  )
  await expect(result.submit(input)).rejects.toThrow('Connection lost')
  await expect(result.submit(input)).resolves.toEqual(accepted)
  expect(requests.map((request) => request.method)).toEqual(['POST', 'PUT', 'POST', 'POST'])
  const manifest: unknown = await requests[0].json()
  expect(manifest).toMatchObject({
    problemId: 'problem-a',
    context: { kind: 'contest', contestId: 'contest-a' },
    files: [
      {
        path: input.filename,
        byteLength: new TextEncoder().encode(input.source).byteLength,
        sha256: 'c72b42440a07cd0ff86ae2c0f1c7cd97780a13f07f00cd3b1f9f87eeaea8de9c'
      }
    ]
  })
  expect(requests[1].credentials).toBe('omit')
  expect(await requests[1].text()).toBe(input.source)
  expect(requests[2].headers.get('Idempotency-Key')).toBe(
    requests[3].headers.get('Idempotency-Key')
  )
  expect(await requests[2].json()).toEqual(await requests[3].json())
})

it('does not upload or accept a source after the target or session has changed', async () => {
  const pending = deferred<Response>()
  const started = deferred<Request>()
  fetcher.mockImplementation((input, init) => {
    started.resolve(new Request(input, init))
    return pending.promise
  })
  const target = ref({ contestId: 'contest-a', problemId: 'problem-a' })
  const { result } = mountQuery(() => useContestSubmission(target), context)
  const saving = result.submit(input)
  const request = await started.promise
  target.value = { ...target.value, problemId: 'problem-b' }
  expect(request.signal.aborted).toBe(true)
  pending.resolve(Response.json(upload))
  await expect(saving).resolves.toBeUndefined()
  await flushPromises()
  expect(fetcher).toHaveBeenCalledTimes(1)
  expect(result.isSubmitting.value).toBe(false)
  context.session.reset()
  await expect(result.submit(input)).rejects.toThrow('参加が必要')
  expect(fetcher).toHaveBeenCalledTimes(1)
})

it('rejects unsafe source paths before starting an upload', async () => {
  const { result } = mountQuery(
    () => useContestSubmission({ contestId: 'contest-a', problemId: 'problem-a' }),
    context
  )
  await expect(result.submit({ ...input, filename: '../main.cpp' })).rejects.toThrow('相対パス')
  expect(fetcher).not.toHaveBeenCalled()
})
