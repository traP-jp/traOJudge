import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { AuthenticationApi, MeApi, ResponseError } from '@/api/generated'
import { queryClient } from '@/api/queryClient'
import { useUserStore } from '@/stores/user'
import { deferred, me } from '@/test/helpers'

beforeEach(() => {
  setActivePinia(createPinia())
  queryClient.clear()
})
afterEach(() => queryClient.clear())

describe('session boundary', () => {
  it('checks the server without a local hint, shares initialization and remembers success', async () => {
    localStorage.clear()
    const response = deferred<typeof me>()
    const get = vi.spyOn(MeApi.prototype, 'getMe').mockReturnValue(response.promise)
    const session = useUserStore()
    const a = session.initialize()
    const b = session.initialize()
    expect(get).toHaveBeenCalledTimes(1)
    response.resolve(me)
    await Promise.all([a, b])
    await session.initialize()
    expect(session.userId).toBe(me.id)
    expect(get).toHaveBeenCalledTimes(1)
  })
  it('allows retry after an initial network failure', async () => {
    const get = vi
      .spyOn(MeApi.prototype, 'getMe')
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce(me)
    const session = useUserStore()
    await session.initialize()
    expect(session.isInitialized).toBe(false)
    expect(session.isLoading).toBe(false)
    expect(session.error).toBeTruthy()
    await session.initialize()
    expect(session.isAuthenticated).toBe(true)
    expect(get).toHaveBeenCalledTimes(2)
  })
  it('settles a 401 as guest without retrying on every navigation', async () => {
    const get = vi
      .spyOn(MeApi.prototype, 'getMe')
      .mockRejectedValue(new ResponseError(new Response(null, { status: 401 })))
    const session = useUserStore()
    await session.initialize()
    await session.initialize()
    expect(session.isInitialized).toBe(true)
    expect(session.error).toBeNull()
    expect(get).toHaveBeenCalledTimes(1)
  })
  it('ignores a late /me after logout and cancels its signal', async () => {
    const response = deferred<typeof me>()
    const get = vi.spyOn(MeApi.prototype, 'getMe').mockReturnValue(response.promise)
    vi.spyOn(AuthenticationApi.prototype, 'postLogout').mockResolvedValue()
    const session = useUserStore()
    const pending = session.initialize()
    const signal = get.mock.calls[0][0] as RequestInit
    await session.logout()
    response.resolve(me)
    await pending
    expect(signal.signal?.aborted).toBe(true)
    expect(session.user).toBeNull()
    expect(session.isInitialized).toBe(true)
  })
  it('ignores an old account response after switching accounts', async () => {
    const old = deferred<typeof me>()
    vi.spyOn(MeApi.prototype, 'getMe')
      .mockReturnValueOnce(old.promise)
      .mockResolvedValueOnce({ ...me, id: 'new-user' })
    const session = useUserStore()
    const pending = session.initialize()
    await session.fetchCurrentUser()
    old.resolve(me)
    await pending
    expect(session.userId).toBe('new-user')
  })
})
