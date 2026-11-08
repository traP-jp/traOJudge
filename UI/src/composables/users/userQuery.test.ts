import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import { useUserQuery } from '@/composables/users/useUserQuery'
import { cleanupQueries, me, mountQuery, setupSession } from '@/test/helpers'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
afterEach(() => {
  cleanupQueries()
  vi.unstubAllGlobals()
})
it('shares public profiles between consumers and switches credentials after login', async () => {
  const context = setupSession()
  context.session.reset()
  const fetcher = vi.fn<(request: Request) => Promise<Response>>().mockImplementation(() =>
    Promise.resolve(
      Response.json({
        id: me.id,
        displayName: 'Alice',
        bio: '紹介',
        githubId: '',
        atcoderId: '',
        xId: '',
        revision: '1'
      })
    )
  )
  vi.stubGlobal('fetch', fetcher)
  const first = mountQuery(() => useUserQuery(me.id), context).result
  await flushPromises()
  const second = mountQuery(() => useUserQuery(me.id), context).result
  await flushPromises()
  expect(fetcher).toHaveBeenCalledTimes(1)
  expect(fetcher.mock.calls[0][0].url).toBe(`http://localhost/api/v1/users/${me.id}`)
  expect(fetcher.mock.calls[0][0].credentials).toBe('omit')
  expect(second.view.value).toMatchObject({
    status: 'success',
    data: { displayName: 'Alice', bio: '紹介' }
  })
  context.session.$patch({ user: me, isInitialized: true })
  await flushPromises()
  expect(fetcher.mock.lastCall?.[0].credentials).toBe('include')
  expect(first.view.value).toMatchObject({ status: 'success', data: { displayName: 'Alice' } })
})
