import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { apiClient } from '@/api/client'
import { ApiError } from '@/api/errors'
import { queryClient } from '@/api/queryClient'
import { useUserStore } from '@/stores/user'
import { deferred, me } from '@/test/helpers'

const profile = (user = me) => ({ data: user, response: Response.json(user) })

beforeEach(() => {
  setActivePinia(createPinia())
  queryClient.clear()
})
afterEach(() => {
  queryClient.clear()
  vi.unstubAllEnvs()
})

describe('session boundary', () => {
  it('checks the server without a local hint, shares initialization and remembers success', async () => {
    localStorage.clear()
    sessionStorage.clear()
    const response = deferred<ReturnType<typeof profile>>()
    const get = vi.spyOn(apiClient, 'GET').mockReturnValue(response.promise)
    const session = useUserStore()
    const a = session.initialize()
    const b = session.initialize()
    expect(get).toHaveBeenCalledTimes(1)
    response.resolve(profile())
    await Promise.all([a, b])
    await session.initialize()
    expect(session.userId).toBe(me.id)
    expect(get).toHaveBeenCalledTimes(1)
  })
  it('allows retry after an initial network failure', async () => {
    const get = vi
      .spyOn(apiClient, 'GET')
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce(profile())
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
      .spyOn(apiClient, 'GET')
      .mockRejectedValue(new ApiError(new Response(null, { status: 401 })))
    const session = useUserStore()
    await session.initialize()
    await session.initialize()
    expect(session.isInitialized).toBe(true)
    expect(session.error).toBeNull()
    expect(get).toHaveBeenCalledTimes(1)
  })
  it('ignores a late /me after logout and cancels its signal', async () => {
    const response = deferred<ReturnType<typeof profile>>()
    const get = vi.spyOn(apiClient, 'GET').mockReturnValue(response.promise)
    vi.spyOn(apiClient, 'DELETE').mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 204 })
    })
    const session = useUserStore()
    const pending = session.initialize()
    const signal = get.mock.calls[0][1] as RequestInit
    await session.logout()
    response.resolve(profile())
    await pending
    expect(signal.signal?.aborted).toBe(true)
    expect(session.user).toBeNull()
    expect(session.isInitialized).toBe(true)
  })
  it('ignores an old account response after switching accounts', async () => {
    const old = deferred<ReturnType<typeof profile>>()
    vi.spyOn(apiClient, 'GET')
      .mockReturnValueOnce(old.promise)
      .mockResolvedValueOnce(profile({ ...me, id: 'new-user' }))
    const session = useUserStore()
    const pending = session.initialize()
    await session.fetchCurrentUser()
    old.resolve(profile())
    await pending
    expect(session.userId).toBe('new-user')
  })
})

it('signs in through the new API and replaces private cached data', async () => {
  const session = useUserStore()
  session.$patch({ user: me, isInitialized: true })
  queryClient.setQueryData(['private'], 'old session data')
  const post = vi.spyOn(apiClient, 'POST').mockResolvedValue({
    data: { status: 'signed-in', session: { userId: me.id, expiresAt: '2099-01-01T00:00:00Z' } },
    response: new Response(null, { status: 200 })
  })
  vi.spyOn(apiClient, 'GET').mockResolvedValue(profile())
  await session.loginWithMock()
  expect(session.user).toEqual(me)
  expect(queryClient.getQueryData(['private'])).toBeUndefined()
  expect(post).toHaveBeenCalledWith(
    '/auth/email/verifications',
    expect.objectContaining({
      body: { token: 'mock-login' },
      params: { header: { Origin: window.location.origin } }
    })
  )
})

it('does not fetch the profile when email verification requires registration', async () => {
  vi.spyOn(apiClient, 'POST').mockResolvedValue({
    data: {
      status: 'registration-required',
      registrationToken: 'test',
      expiresAt: '2099-01-01T00:00:00Z'
    },
    response: new Response(null, { status: 200 })
  })
  const get = vi.spyOn(apiClient, 'GET')
  const session = useUserStore()
  await expect(session.loginWithMock()).rejects.toThrow()
  expect(get).not.toHaveBeenCalled()
  expect(session.isAuthenticated).toBe(false)
})

it('rejects a late sign-in after logout without restoring the account', async () => {
  const pending = deferred<never>()
  vi.spyOn(apiClient, 'POST').mockReturnValue(pending.promise)
  const get = vi.spyOn(apiClient, 'GET')
  vi.spyOn(apiClient, 'DELETE').mockResolvedValue({
    data: undefined,
    response: new Response(null, { status: 204 })
  })
  const session = useUserStore()
  const login = session.loginWithMock()
  await session.logout()
  pending.reject(new ApiError(new Response(null, { status: 503 })))
  await expect(login).rejects.toThrow()
  expect(get).not.toHaveBeenCalled()
  expect(session.isInitialized).toBe(true)
  expect(session.error).toBeNull()
  expect(session.user).toBeNull()
})

it('reports logout failure and leaves the session available for rechecking', async () => {
  const session = useUserStore()
  session.$patch({ user: me, isInitialized: true })
  queryClient.setQueryData(['private'], 'cached data')
  vi.spyOn(apiClient, 'DELETE').mockRejectedValue(new Error('offline'))
  await expect(session.logout()).rejects.toThrow('offline')
  expect(session.isInitialized).toBe(false)
  expect(session.error).toBeTruthy()
  expect(queryClient.getQueryData(['private'])).toBeUndefined()
  vi.spyOn(apiClient, 'GET').mockResolvedValue(profile())
  await session.initialize()
  expect(session.isAuthenticated).toBe(true)
})

it('does not send mock credentials in production', async () => {
  vi.stubEnv('DEV', false)
  const post = vi.spyOn(apiClient, 'POST')
  await expect(useUserStore().loginWithMock()).rejects.toThrow('開発環境')
  expect(post).not.toHaveBeenCalled()
})
