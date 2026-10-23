import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createApp } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import { Oauth2Api } from '@/api/generated'
import { useOAuthStore } from '@/stores/oauth'
import { deferred } from '@/test/helpers'

beforeEach(() => {
  sessionStorage.clear()
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
})
afterEach(() => sessionStorage.clear())

const setup = async () => {
  const pinia = createPinia()
  setActivePinia(pinia)
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/:pathMatch(.*)*', component: { render: () => null } }]
  })
  const app = createApp({ render: () => null })
  app.use(pinia).use(router)
  await router.push('/')
  return { router, store: app.runWithContext(() => useOAuthStore(pinia)) }
}

it.each([
  ['null', () => null],
  ['missing fields', () => ({})],
  ['unknown provider', () => ({ provider: 'other', action: 'login', timestamp: Date.now() })],
  ['unknown action', () => ({ provider: 'google', action: 'other', timestamp: Date.now() })],
  ['invalid timestamp', () => ({ provider: 'google', action: 'login', timestamp: 'invalid' })],
  [
    'invalid redirect',
    () => ({ provider: 'google', action: 'login', timestamp: Date.now(), redirectTo: [] })
  ]
])('rejects malformed saved OAuth state before contacting the API: %s', async (_, state) => {
  const { store } = await setup()
  const authorize = vi.spyOn(Oauth2Api.prototype, 'postGoogleOAuthAuthorize')
  sessionStorage.setItem('oauth_state_test', JSON.stringify(state()))
  await expect(store.handleOAuthCallback('code', 'test')).rejects.toThrow('Invalid OAuth state')
  expect(authorize).not.toHaveBeenCalled()
  expect(store.isOAuthInProgress).toBe(false)
  expect(store.oauthError).toBe('Invalid OAuth state')
})

it('rejects invalid JSON before contacting the API', async () => {
  const { store } = await setup()
  const authorize = vi.spyOn(Oauth2Api.prototype, 'postGoogleOAuthAuthorize')
  sessionStorage.setItem('oauth_state_test', '{')
  await expect(store.handleOAuthCallback('code', 'test')).rejects.toBeInstanceOf(SyntaxError)
  expect(authorize).not.toHaveBeenCalled()
  expect(store.isOAuthInProgress).toBe(false)
})

it('rejects an expired callback without contacting the API', async () => {
  const { store } = await setup()
  const authorize = vi.spyOn(Oauth2Api.prototype, 'postGoogleOAuthAuthorize')
  sessionStorage.setItem(
    'oauth_state_test',
    JSON.stringify({
      provider: 'google',
      action: 'login',
      timestamp: Date.now() - 11 * 60 * 1000
    })
  )
  await expect(store.handleOAuthCallback('code', 'test')).rejects.toThrow('OAuth state expired')
  expect(authorize).not.toHaveBeenCalled()
  expect(sessionStorage.getItem('oauth_state_test')).toBeNull()
})

it('cleans up malformed and expired entries while keeping current state', async () => {
  const current = { provider: 'google', action: 'signup', timestamp: Date.now() }
  sessionStorage.setItem('oauth_state_current', JSON.stringify(current))
  sessionStorage.setItem('oauth_state_expired', JSON.stringify({ ...current, timestamp: 0 }))
  sessionStorage.setItem('oauth_state_bad', 'null')
  sessionStorage.setItem('other-key', 'keep')
  await setup()
  expect(sessionStorage.getItem('oauth_state_current')).toBe(JSON.stringify(current))
  expect(sessionStorage.getItem('oauth_state_expired')).toBeNull()
  expect(sessionStorage.getItem('oauth_state_bad')).toBeNull()
  expect(sessionStorage.getItem('other-key')).toBe('keep')
})

it('waits for signup navigation before completing the callback', async () => {
  const { store, router } = await setup()
  sessionStorage.setItem(
    'oauth_state_test',
    JSON.stringify({
      provider: 'google',
      action: 'signup',
      timestamp: Date.now()
    })
  )
  vi.spyOn(Oauth2Api.prototype, 'postGoogleOAuthAuthorize').mockResolvedValue({ token: 'a+b' })
  const navigation = deferred<void>()
  const push = vi.spyOn(router, 'push').mockReturnValue(navigation.promise)
  const callback = store.handleOAuthCallback('code', 'test')
  await vi.waitFor(() =>
    expect(push).toHaveBeenCalledWith('/signup/register?oauth=true&token=a%2Bb')
  )
  expect(store.isOAuthInProgress).toBe(true)
  expect(sessionStorage.getItem('oauth_state_test')).not.toBeNull()
  navigation.resolve()
  await callback
  expect(store.isOAuthInProgress).toBe(false)
  expect(sessionStorage.getItem('oauth_state_test')).toBeNull()
})
