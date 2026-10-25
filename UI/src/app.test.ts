import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import { AuthenticationApi, MeApi, Oauth2Api, ResponseError } from '@/api/generated'
import { ContestsApi } from '@/api/generated/contest/apis/ContestsApi'
import { queryClient } from '@/api/queryClient'
import App from '@/App.vue'
import PrimaryButton from '@/components/controls/PrimaryButton.vue'
import appRouter, { watchSessionRouting } from '@/router'
import { useUserStore } from '@/stores/user'
import { deferred, me } from '@/test/helpers'
import LoginView from '@/views/LoginView.vue'
import OAuthCallbackView from '@/views/OAuthCallbackView.vue'

const wrappers: Pick<VueWrapper, 'unmount'>[] = []
afterEach(() => {
  wrappers.splice(0).forEach((w) => w.unmount())
  queryClient.clear()
  sessionStorage.clear()
})
async function setup(path: string) {
  const pinia = createPinia()
  setActivePinia(pinia)
  const session = useUserStore()
  session.$patch({ isInitialized: true })
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/login', component: LoginView },
      { path: '/oauth/:provider/:action/callback', component: OAuthCallbackView },
      { path: '/problems', component: { template: '<p>Problems</p>' } },
      { path: '/', component: { template: '<p>Home</p>' } },
      { path: '/contest/:contestId', component: { template: '<p>Public contest</p>' } }
    ]
  })
  await router.push(path)
  const wrapper = mount(App, {
    global: {
      plugins: [pinia, router],
      stubs: { TopNavbar: true, BottomNavbar: true, OAuthButton: true }
    }
  })
  wrappers.push(wrapper)
  return { wrapper, session, router }
}

it('keeps the login error visible after a real delayed 401', async () => {
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
  const pending = deferred<void>()
  vi.spyOn(AuthenticationApi.prototype, 'postLogin').mockReturnValue(pending.promise)
  const { wrapper } = await setup('/login')
  await wrapper.get('input[placeholder="メールアドレス"]').setValue('alice@example.com')
  await wrapper.get('input[type=password]').setValue('wrong-password')
  await wrapper.getComponent(PrimaryButton).trigger('click')
  await flushPromises()
  pending.reject(new ResponseError(new Response(null, { status: 401 })))
  await flushPromises()
  expect(wrapper.text()).toContain('メールアドレスまたはパスワードが正しくありません。')
})

it('runs the OAuth authorize call once across a delayed /me', async () => {
  const pending = deferred<typeof me>()
  const authorize = vi
    .spyOn(Oauth2Api.prototype, 'postTraqOAuthAuthorize')
    .mockResolvedValue(undefined)
  vi.spyOn(MeApi.prototype, 'getMe').mockReturnValue(pending.promise)
  const { wrapper, router } = await setup('/oauth/traq/login/callback?redirect=/problems')
  await flushPromises()
  expect(wrapper.findComponent({ name: 'OAuthCallbackView' }).exists()).toBe(true)
  pending.resolve(me)
  await flushPromises()
  expect(authorize).toHaveBeenCalledTimes(1)
  expect(router.currentRoute.value.path).toBe('/problems')
})

it.each([401, 503])(
  'keeps the real public contest visible before and after a legacy %i',
  async (status) => {
    const pinia = createPinia()
    setActivePinia(pinia)
    const pending = deferred<typeof me>()
    vi.spyOn(MeApi.prototype, 'getMe').mockReturnValue(pending.promise)
    const getContest = vi.spyOn(ContestsApi.prototype, 'getContest').mockResolvedValue({
      id: '1',
      title: 'Public contest',
      description: '- Overview',
      visibility: 'public',
      startsAt: new Date('2026-01-01T00:00:00Z'),
      endsAt: new Date('2026-01-01T01:00:00Z'),
      rules: { tieBreak: 'icpc', penaltyMinutes: 5 },
      revision: '1'
    })
    try {
      await appRouter.push({ path: '/contest/1', force: true })
      const wrapper = mount(App, {
        global: { plugins: [pinia, appRouter], stubs: { TopNavbar: true, BottomNavbar: true } }
      })
      wrappers.push(wrapper)
      await flushPromises()
      expect(wrapper.get('h1').text()).toBe('Public contest')
      expect(useUserStore().isInitialized).toBe(false)
      pending.reject(new ResponseError(new Response(null, { status })))
      await flushPromises()
      expect(wrapper.get('h1').text()).toBe('Public contest')
      expect(appRouter.currentRoute.value.path).toBe('/contest/1')
      expect(getContest).toHaveBeenCalledTimes(1)
    } finally {
      pending.resolve(me)
      await flushPromises()
    }
  }
)

it('does not mount protected content while the session is unknown and supports retry', async () => {
  const pinia = createPinia()
  setActivePinia(pinia)
  const getMe = vi.spyOn(MeApi.prototype, 'getMe').mockRejectedValue(new Error('offline'))
  const stop = watchSessionRouting()
  const remove = appRouter.addRoute({
    path: '/private-test',
    meta: { requiresAuth: true },
    component: { template: '<p>Private content</p>' }
  })
  try {
    await appRouter.push('/private-test')
    const wrapper = mount(App, {
      global: { plugins: [pinia, appRouter], stubs: { TopNavbar: true, BottomNavbar: true } }
    })
    wrappers.push(wrapper)
    expect(wrapper.text()).not.toContain('Private content')
    getMe.mockResolvedValue(me)
    await wrapper.get('button').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('Private content')
    useUserStore().reset()
    await flushPromises()
    expect(wrapper.text()).not.toContain('Private content')
    await vi.waitFor(() => expect(appRouter.currentRoute.value.path).toBe('/login'))
    expect(appRouter.currentRoute.value.query.redirect).toBe('/private-test')
  } finally {
    stop()
    remove()
  }
})
