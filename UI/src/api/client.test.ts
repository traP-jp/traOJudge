import { afterEach, expect, it, vi } from 'vitest'

import { apiClient } from '@/api/client'
import { ApiError } from '@/api/errors'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
afterEach(() => vi.unstubAllGlobals())

it('preserves role scope IDs, cursor values and ISO dates without conversion', async () => {
  const role = {
    role: 'contest-setter',
    scope: { kind: 'contest', contestId: 'contest-a' },
    userId: 'user-a',
    grantedAt: '2026-01-01T00:00:00Z'
  }
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(Response.json({ items: [role] })))
  const roles = await apiClient.GET('/me/roles')
  expect(roles.data?.items[0]).toEqual(role)
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue(Response.json({ items: [], nextCursor: 'opaque+/cursor=' }))
  )
  const list = await apiClient.GET('/problems/{problemId}/editorials', {
    params: { path: { problemId: 'a' }, query: { cursor: 'opaque+/cursor=' } }
  })
  expect(list.data?.nextCursor).toBe('opaque+/cursor=')
})

it('passes credentials, cancellation and escaped path/query values to fetch', async () => {
  const fetcher = vi
    .fn<(request: Request) => Promise<Response>>()
    .mockResolvedValue(Response.json({ items: [] }))
  vi.stubGlobal('fetch', fetcher)
  const controller = new AbortController()
  await apiClient.GET('/problems/{problemId}/editorials', {
    params: { path: { problemId: 'a/b' }, query: { cursor: 'next+page' } },
    signal: controller.signal
  })
  const request = fetcher.mock.calls[0][0]
  expect(request.url).toContain('/problems/a%2Fb/editorials?cursor=next%2Bpage')
  expect(request.credentials).toBe('include')
  controller.abort()
  expect(request.signal.aborted).toBe(true)
})

it('reports HTTP status even when the error response is not JSON', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue(new Response('<html>Denied</html>', { status: 403 }))
  )
  await expect(apiClient.GET('/me/roles')).rejects.toBeInstanceOf(ApiError)
})
