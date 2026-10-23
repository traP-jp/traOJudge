import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'

import { ProblemsApi, ResponseError } from '@/api/generated'
import { queryClient } from '@/api/queryClient'
import { useProblemQuery } from '@/composables/problems/useProblemQuery'
import { useProblemsQuery } from '@/composables/problems/useProblemsQuery'
import { cleanupQueries, deferred, me, mountQuery, problem, setupSession } from '@/test/helpers'

let context: ReturnType<typeof setupSession>
beforeEach(() => {
  context = setupSession()
})
afterEach(cleanupQueries)

describe('problem queries', () => {
  it('shares a pending request and reuses it for a later mounted child', async () => {
    const response = deferred<ReturnType<typeof problem>>()
    const get = vi.spyOn(ProblemsApi.prototype, 'getProblem').mockReturnValue(response.promise)
    const first = mountQuery(() => useProblemQuery('a'), context)
    const second = mountQuery(() => useProblemQuery('a'), context)
    expect(get).toHaveBeenCalledTimes(1)
    response.resolve(problem('a'))
    await flushPromises()
    expect(first.result.view.value).toMatchObject({ status: 'success', data: { id: 'a' } })
    expect(second.result.view.value).toMatchObject({ status: 'success' })
    mountQuery(() => useProblemQuery('a'), context)
    await flushPromises()
    expect(get).toHaveBeenCalledTimes(1)
  })

  it('cancels A and keeps B when A finishes last', async () => {
    const a = deferred<ReturnType<typeof problem>>()
    const b = deferred<ReturnType<typeof problem>>()
    const get = vi
      .spyOn(ProblemsApi.prototype, 'getProblem')
      .mockReturnValueOnce(a.promise)
      .mockReturnValueOnce(b.promise)
    const id = ref('a')
    const { result } = mountQuery(() => useProblemQuery(id), context)
    const signal = get.mock.calls[0][1] as RequestInit
    id.value = 'b'
    await nextTick()
    expect(signal.signal?.aborted).toBe(true)
    expect(result.view.value.status).toBe('pending')
    b.resolve(problem('b'))
    await flushPromises()
    a.resolve(problem('a'))
    await flushPromises()
    expect(result.view.value).toMatchObject({ status: 'success', data: { id: 'b' } })
  })

  it.each([403, 404])('hides cached content after %i and permits retry', async (status) => {
    const get = vi.spyOn(ProblemsApi.prototype, 'getProblem').mockResolvedValue(problem('a'))
    const { result } = mountQuery(() => useProblemQuery('a'), context)
    await flushPromises()
    get.mockRejectedValueOnce(new ResponseError(new Response(null, { status })))
    await result.refresh()
    expect(result.view.value.status).toBe('error')
    expect(result.view.value).not.toHaveProperty('data')
    await result.refresh()
    expect(result.view.value.status).toBe('success')
  })

  it('expires the session and removes cached data after 401', async () => {
    const get = vi.spyOn(ProblemsApi.prototype, 'getProblem').mockResolvedValue(problem('a'))
    const { result } = mountQuery(() => useProblemQuery('a'), context)
    await flushPromises()
    get.mockRejectedValueOnce(new ResponseError(new Response(null, { status: 401 })))
    await result.refresh()
    expect(context.session.isAuthenticated).toBe(false)
    expect(result.view.value).not.toHaveProperty('data')
    expect(
      queryClient
        .getQueryCache()
        .getAll()
        .every((query) => query.state.data === undefined)
    ).toBe(true)
  })

  it('shows a retryable initial failure and a warning for a failed refresh', async () => {
    const get = vi
      .spyOn(ProblemsApi.prototype, 'getProblem')
      .mockRejectedValueOnce(new Error('offline'))
    const { result } = mountQuery(() => useProblemQuery('a'), context)
    await flushPromises()
    expect(result.view.value.status).toBe('error')
    get.mockResolvedValueOnce(problem('a'))
    await result.refresh()
    get.mockRejectedValueOnce(new ResponseError(new Response(null, { status: 500 })))
    await result.refresh()
    expect(result.view.value).toMatchObject({
      status: 'success',
      data: { id: 'a' }
    })
    expect(result.view.value).toHaveProperty('warning', expect.any(String))
  })

  it('does not restore content or caches after logout during a request', async () => {
    const response = deferred<ReturnType<typeof problem>>()
    vi.spyOn(ProblemsApi.prototype, 'getProblem').mockReturnValue(response.promise)
    const { result } = mountQuery(() => useProblemQuery('a'), context)
    context.session.reset()
    response.resolve(problem('a'))
    await flushPromises()
    expect(result.view.value.status).toBe('pending')
    expect(
      queryClient
        .getQueryCache()
        .getAll()
        .every((query) => query.state.data === undefined)
    ).toBe(true)
  })

  it('ignores an old session 401 after switching accounts', async () => {
    const old = deferred<ReturnType<typeof problem>>()
    vi.spyOn(ProblemsApi.prototype, 'getProblem')
      .mockReturnValueOnce(old.promise)
      .mockResolvedValue(problem('a'))
    const { result } = mountQuery(() => useProblemQuery('a'), context)
    context.session.reset()
    context.session.$patch({
      user: { ...me, id: 'new-user' },
      isInitialized: true
    })
    await flushPromises()
    old.reject(new ResponseError(new Response(null, { status: 401 })))
    await flushPromises()
    expect(context.session.userId).toBe('new-user')
    expect(context.session.isAuthenticated).toBe(true)
    expect(result.view.value).toMatchObject({ status: 'success', data: { id: 'a' } })
  })

  it('keeps empty results on page 1 and follows the user target', async () => {
    const get = vi
      .spyOn(ProblemsApi.prototype, 'getProblems')
      .mockResolvedValue({ problems: [], total: 0 })
    const id = ref('a')
    const { result } = mountQuery(
      () =>
        useProblemsQuery(() => ({ kind: 'user', userId: id.value }), {
          page: 1,
          difficultyBegin: 1,
          difficultyEnd: 10
        }),
      context
    )
    await flushPromises()
    expect(result.view.value).toMatchObject({
      status: 'success',
      data: { problems: [], totalPages: 1 }
    })
    id.value = 'b'
    await flushPromises()
    expect(get.mock.lastCall?.[0]).toMatchObject({ userId: 'b', offset: 0 })
  })
})
