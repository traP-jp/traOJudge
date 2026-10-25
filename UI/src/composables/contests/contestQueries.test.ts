import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'

import type { Contest, Editorial } from '@/api/client'
import { useContestQuery } from '@/composables/contests/useContestQuery'
import { useEditorialQuery, useEditorialsQuery } from '@/composables/contests/useEditorialsQuery'
import { cleanupQueries, deferred, me, mountQuery, setupSession } from '@/test/helpers'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
let context: ReturnType<typeof setupSession>
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
const editorial: Editorial = {
  id: 'editorial-a',
  problemId: 'problem-a',
  authorId: me.id,
  title: 'Editorial',
  body: 'Secret body',
  visibility: 'private',
  revision: '1',
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z'
}
const fetcher = vi.fn<(request: Request) => Promise<Response>>()
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
const reply = (request: Request) => {
  const url = new URL(request.url)
  if (url.pathname.endsWith('/me/roles')) return Response.json({ items: [] })
  if (url.pathname.endsWith('/contests/contest-a')) return Response.json(contest)
  if (url.pathname.endsWith('/contests/contest-a/problems'))
    return Response.json({ items: [{ problemId: 'problem-a', label: 'A', order: 0 }] })
  if (url.pathname.endsWith('/problems/problem-a')) return Response.json({ title: 'Problem A' })
  if (url.pathname.endsWith('/problems/problem-a/editorials'))
    return Response.json({
      items: request.credentials === 'include' ? [editorial] : [],
      nextCursor: null
    })
  return Response.json(editorial)
}

it('loads public contests while authentication is pending and switches viewers', async () => {
  context.session.$patch({ user: null, isInitialized: false, isLoading: true })
  fetcher.mockImplementation((request) => Promise.resolve(reply(request)))
  const { result } = mountQuery(() => useContestQuery('contest-a'), context)
  await flushPromises()
  expect(result.view.value).toMatchObject({ status: 'success', data: contest })
  expect(fetcher.mock.calls[0][0].credentials).toBe('omit')
  context.session.$patch({ user: me, isInitialized: true, isLoading: false })
  await flushPromises()
  expect(fetcher.mock.lastCall?.[0].credentials).toBe('include')
})

it('cancels the previous contest and ignores its late response', async () => {
  const pending = deferred<Response>()
  fetcher
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValue(Response.json({ ...contest, id: 'contest-b' }))
  const id = ref('contest-a')
  const { result } = mountQuery(() => useContestQuery(id), context)
  await flushPromises()
  const request = fetcher.mock.calls[0][0]
  id.value = 'contest-b'
  await nextTick()
  expect(request.signal.aborted).toBe(true)
  await flushPromises()
  pending.resolve(Response.json(contest))
  await flushPromises()
  expect(result.view.value).toMatchObject({ status: 'success', data: { id: 'contest-b' } })
})

it('removes an author’s private editorial immediately on logout', async () => {
  fetcher.mockImplementation((request) => Promise.resolve(reply(request)))
  const { result } = mountQuery(() => useEditorialsQuery(contest), context)
  await flushPromises()
  expect(result.view.value).toMatchObject({
    status: 'success',
    data: { problems: [{ editorials: [editorial] }] }
  })
  context.session.reset()
  await nextTick()
  expect(result.view.value).not.toHaveProperty('data.problems.0.editorials.0')
  await flushPromises()
  expect(result.view.value).toMatchObject({
    status: 'success',
    data: { problems: [{ editorials: [] }] }
  })
})

it('keeps guest editorial bodies unfetched until release and refreshes at the end', async () => {
  vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] })
  vi.setSystemTime(new Date('2026-01-01T00:59:59Z'))
  context.session.reset()
  fetcher.mockImplementation((request) => Promise.resolve(reply(request)))
  const { result } = mountQuery(() => useEditorialsQuery(contest), context)
  await flushPromises()
  expect(result.view.value).toMatchObject({ status: 'success', data: { awaitingRelease: true } })
  expect(fetcher).not.toHaveBeenCalled()
  await vi.advanceTimersByTimeAsync(1000)
  await flushPromises()
  expect(result.view.value).toMatchObject({ status: 'success', data: { awaitingRelease: false } })
  expect(fetcher).toHaveBeenCalled()
})

it('follows cursor pages and handles a repeated cursor without hanging', async () => {
  context.session.reset()
  fetcher.mockImplementation((request) => {
    const url = new URL(request.url)
    if (!url.pathname.endsWith('/editorials')) return Promise.resolve(reply(request))
    return Promise.resolve(
      Response.json({
        items: [
          { ...editorial, id: url.searchParams.get('cursor') || 'first', visibility: 'public' }
        ],
        nextCursor: url.searchParams.has('cursor') ? null : 'next'
      })
    )
  })
  const { result } = mountQuery(() => useEditorialsQuery(contest), context)
  await flushPromises()
  expect(result.view.value).toMatchObject({
    status: 'success',
    data: { problems: [{ editorials: [{ id: 'first' }, { id: 'next' }], error: false }] }
  })
  fetcher.mockImplementation((request) =>
    Promise.resolve(
      request.url.includes('/editorials')
        ? Response.json({ items: [], nextCursor: 'repeat' })
        : reply(request)
    )
  )
  await result.refresh()
  expect(result.view.value).toMatchObject({
    status: 'success',
    data: { problems: [{ error: true }] }
  })
})

it('rejects a detail URL whose problem is outside the contest', async () => {
  fetcher.mockImplementation((request) => Promise.resolve(reply(request)))
  const { result } = mountQuery(
    () => useEditorialQuery(contest, 'wrong-problem', editorial.id),
    context
  )
  await flushPromises()
  expect(result.view.value.status).toBe('error')
  expect(
    fetcher.mock.calls.some(([request]) => request.url.endsWith('/editorials/editorial-a'))
  ).toBe(false)
})

it.each([403, 404])('hides previously available contest data after HTTP %i', async (status) => {
  fetcher.mockResolvedValueOnce(Response.json(contest))
  const { result } = mountQuery(() => useContestQuery(contest.id), context)
  await flushPromises()
  fetcher.mockResolvedValue(new Response(null, { status }))
  await result.refresh()
  expect(result.view.value.status).toBe('error')
})

it('does not apply a late unauthorized response to a new session', async () => {
  const pending = deferred<Response>()
  fetcher
    .mockReturnValueOnce(pending.promise)
    .mockImplementation(() => Promise.resolve(Response.json(contest)))
  const { result } = mountQuery(() => useContestQuery(contest.id), context)
  await flushPromises()
  context.session.reset()
  context.session.$patch({ user: { ...me, id: 'new-viewer' }, isInitialized: true })
  await flushPromises()
  pending.resolve(new Response(null, { status: 401 }))
  await flushPromises()
  expect(context.session.userId).toBe('new-viewer')
  expect(result.view.value.status).toBe('success')
})

it('keeps released editorials readable if roles fail, but preserves pre-release errors', async () => {
  fetcher.mockImplementation((request) =>
    Promise.resolve(
      request.url.endsWith('/me/roles') ? new Response(null, { status: 500 }) : reply(request)
    )
  )
  const value = ref(contest)
  const { result } = mountQuery(() => useEditorialsQuery(value), context)
  await flushPromises()
  expect(result.view.value.status).toBe('success')
  value.value = { ...contest, endsAt: '2099-01-01T00:00:00Z' }
  await flushPromises()
  expect(result.view.value.status).toBe('error')
})

it.each(['invalid-date', contest.startsAt])(
  'rejects an invalid contest end: %s',
  async (endsAt) => {
    fetcher.mockResolvedValue(Response.json({ ...contest, endsAt }))
    const { result } = mountQuery(() => useContestQuery(contest.id), context)
    await flushPromises()
    expect(result.view.value.status).toBe('error')
  }
)
