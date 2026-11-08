import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import type { Clarification } from '@/api/client'
import { useClarificationActions } from '@/composables/contests/useClarificationActions'
import { cleanupQueries, deferred, me, mountQuery, setupSession } from '@/test/helpers'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
const fetcher = vi.fn<typeof fetch>()
const question: Clarification = {
  id: 'question-a',
  authorId: me.id,
  body: '質問',
  problemId: null,
  answer: null,
  isPublic: false,
  answeredBy: null,
  revision: '1',
  createdAt: '2026-01-01T00:00:00Z'
}
let context: ReturnType<typeof setupSession>
beforeEach(() => {
  context = setupSession()
  context.session.user = {
    ...me,
    roles: (['contestant', 'contest-setter'] as const).map((role) => ({
      role,
      scope: { kind: 'contest', contestId: 'contest-a' },
      userId: me.id,
      grantedAt: question.createdAt
    }))
  }
  fetcher.mockReset()
  vi.stubGlobal('fetch', fetcher)
})
afterEach(() => {
  cleanupQueries()
  vi.unstubAllGlobals()
})

it('reuses a creation key after an uncertain response and blocks an unassigned contest', async () => {
  const requests: Request[] = []
  fetcher.mockImplementation((input, init) => {
    requests.push(new Request(input, init))
    return requests.length === 1
      ? Promise.reject(new TypeError('connection lost'))
      : Promise.resolve(Response.json(question))
  })
  const id = ref('contest-a')
  const { result } = mountQuery(() => useClarificationActions(id), context)
  await expect(result.create('質問')).rejects.toThrow('connection lost')
  await expect(result.create('質問')).resolves.toEqual(question)
  expect(requests[0].headers.get('Idempotency-Key')).toBe(
    requests[1].headers.get('Idempotency-Key')
  )
  id.value = 'contest-b'
  await expect(result.create('質問')).rejects.toThrow('権限')
  expect(requests).toHaveLength(2)
})

it('uses the server ETag and recognizes a successful retry without overwriting another answer', async () => {
  const saved = { ...question, answer: '回答', isPublic: true }
  const requests: Request[] = []
  fetcher.mockImplementation((input, init) => {
    const request = new Request(input, init)
    requests.push(request)
    if (request.method === 'PUT') return Promise.reject(new TypeError('response lost'))
    return Promise.resolve(
      Response.json(requests.length === 1 ? question : saved, {
        headers: { ETag: '"server-revision"' }
      })
    )
  })
  const { result } = mountQuery(() => useClarificationActions('contest-a'), context)
  await expect(result.answer(question.id, '回答', true)).rejects.toThrow('response lost')
  expect(requests[1].headers.get('If-Match')).toBe('"server-revision"')
  expect(await requests[1].json()).toEqual({ body: '回答', isPublic: true })
  await expect(result.answer(question.id, '回答', true)).resolves.toEqual(saved)
  await expect(result.answer(question.id, '別の回答', false)).rejects.toThrow('既に回答')
  expect(requests.filter((request) => request.method === 'PUT')).toHaveLength(1)
})

it('does not save an answer after logout while the revision is being fetched', async () => {
  const pending = deferred<Response>()
  fetcher.mockReturnValue(pending.promise)
  const { result } = mountQuery(() => useClarificationActions('contest-a'), context)
  const saving = result.answer(question.id, '回答', false)
  context.session.reset()
  pending.resolve(Response.json(question, { headers: { ETag: '"1"' } }))
  await expect(saving).resolves.toBeUndefined()
  expect(fetcher).toHaveBeenCalledTimes(1)
  expect(result.isSaving.value).toBe(false)
})
