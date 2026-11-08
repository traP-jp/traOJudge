import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import type { Contest, Problem } from '@/api/client'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import { cleanupQueries, deferred, me, mountQuery, setupSession } from '@/test/helpers'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
const contest: Contest = {
  id: 'contest-a',
  title: 'Contest A',
  visibility: 'public',
  participationMode: 'individual',
  startsAt: '2026-01-01T00:00:00Z',
  endsAt: '2026-01-01T01:00:00Z',
  rules: { tieBreak: 'icpc', penaltyMinutes: 5 },
  revision: '1'
}
const problem = (id: string): Problem => ({
  id,
  title: id,
  statement: '本文',
  visibility: 'public',
  revision: '1',
  createdByUserId: me.id,
  currentVersionId: null,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z'
})
const fetcher = vi.fn<(request: Request) => Promise<Response>>()
let context: ReturnType<typeof setupSession>
beforeEach(() => {
  context = setupSession()
  fetcher.mockReset()
  vi.stubGlobal('fetch', fetcher)
})
afterEach(() => {
  cleanupQueries()
  vi.unstubAllGlobals()
  vi.useRealTimers()
})
const reply = (request: Request) =>
  request.url.endsWith('/problems')
    ? Response.json({ items: [{ problemId: 'problem-a', label: 'A', order: 0 }] })
    : Response.json(problem('problem-a'))

it('keeps problems unfetched before the start and releases them without a reload', async () => {
  vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] })
  vi.setSystemTime(new Date('2025-12-31T23:59:59Z'))
  context.session.reset()
  fetcher.mockImplementation((request) => Promise.resolve(reply(request)))
  const { result } = mountQuery(() => useContestProblemsQuery(contest), context)
  await flushPromises()
  expect(result.view.value).toMatchObject({ status: 'success', data: { awaitingStart: true } })
  expect(fetcher).not.toHaveBeenCalled()
  await vi.advanceTimersByTimeAsync(1000)
  await flushPromises()
  expect(result.view.value).toMatchObject({
    data: { awaitingStart: false, problems: [{ problem: problem('problem-a') }] }
  })
  expect(fetcher.mock.calls.every(([request]) => request.credentials === 'omit')).toBe(true)
})

it('uses the setter role only for its assigned contest', async () => {
  vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] })
  vi.setSystemTime(new Date('2025-12-31T23:59:59Z'))
  context.session.user = {
    ...me,
    roles: [
      {
        role: 'contest-setter',
        scope: { kind: 'contest', contestId: 'other' },
        userId: me.id,
        grantedAt: '2025-01-01T00:00:00Z'
      }
    ]
  }
  fetcher.mockImplementation((request) => Promise.resolve(reply(request)))
  const { result } = mountQuery(() => useContestProblemsQuery(contest), context)
  await flushPromises()
  expect(fetcher).not.toHaveBeenCalled()
  context.session.user.roles[0].scope = { kind: 'contest', contestId: contest.id }
  await flushPromises()
  expect(result.view.value).toMatchObject({ data: { awaitingStart: false } })
})

it('sorts the problem list and preserves readable problems when another is forbidden', async () => {
  fetcher.mockImplementation((request) => {
    if (request.url.endsWith('/problems'))
      return Promise.resolve(
        Response.json({
          items: [
            { problemId: 'problem-b', label: 'B', order: 1 },
            { problemId: 'problem-a', label: 'A', order: 0 }
          ]
        })
      )
    return Promise.resolve(
      request.url.endsWith('problem-b') ? new Response(null, { status: 403 }) : reply(request)
    )
  })
  const { result } = mountQuery(() => useContestProblemsQuery(contest), context)
  await flushPromises()
  expect(result.view.value).toMatchObject({
    data: {
      problems: [
        { problemId: 'problem-a', error: false },
        { problemId: 'problem-b', problem: undefined, error: true }
      ]
    }
  })
})

it('does not restore a private statement from a request completed after logout', async () => {
  const pending = deferred<Response>()
  fetcher.mockImplementation((request) => {
    if (request.url.endsWith('/problems')) return Promise.resolve(reply(request))
    return request.credentials === 'include'
      ? pending.promise
      : Promise.resolve(new Response(null, { status: 403 }))
  })
  const { result } = mountQuery(() => useContestProblemsQuery(contest), context)
  await flushPromises()
  const request = fetcher.mock.calls[1][0]
  context.session.reset()
  await nextTick()
  expect(request.signal.aborted).toBe(true)
  pending.resolve(Response.json({ ...problem('problem-a'), statement: '非公開の本文' }))
  await flushPromises()
  expect(result.view.value).toMatchObject({
    data: { problems: [{ problem: undefined, error: true }] }
  })
})
