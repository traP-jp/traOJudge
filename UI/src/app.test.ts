import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import { apiClient } from '@/api/client'
import { ApiError } from '@/api/errors'
import { queryClient } from '@/api/queryClient'
import App from '@/App.vue'
import PrimaryButton from '@/components/controls/PrimaryButton.vue'
import appRouter, { watchSessionRouting } from '@/router'
import { useUserStore } from '@/stores/user'
import { deferred, me } from '@/test/helpers'
import AuthTodoView from '@/views/AuthTodoView.vue'
import LoginView from '@/views/LoginView.vue'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))

const wrappers: Pick<VueWrapper, 'unmount'>[] = []
afterEach(() => {
  wrappers.splice(0).forEach((w) => w.unmount())
  queryClient.clear()
  sessionStorage.clear()
  vi.unstubAllGlobals()
})
async function setup(path: string) {
  const pinia = createPinia()
  setActivePinia(pinia)
  const session = useUserStore()
  session.$patch({ isInitialized: true })
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- ESLintでは、.vueからimportしたコンポーネントの型を解決できないため。
      { path: '/login', component: LoginView },
      // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- ESLintでは、.vueからimportしたコンポーネントの型を解決できないため。
      { path: '/oauth/:pathMatch(.*)*', component: AuthTodoView },
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

it('keeps a delayed mock login failure visible', async () => {
  const pending = deferred<never>()
  vi.spyOn(apiClient, 'POST').mockReturnValue(pending.promise)
  const { wrapper } = await setup('/login')
  await wrapper.getComponent(PrimaryButton).trigger('click')
  pending.reject(new ApiError(new Response(null, { status: 401 })))
  await flushPromises()
  expect(wrapper.text()).toContain('モックでログインできませんでした')
})

it('shows the placeholder for removed external authentication routes', async () => {
  const { wrapper } = await setup('/oauth/traq/login/callback')
  expect(wrapper.text()).toContain('認証機能は準備中です')
})

it('keeps mock login unavailable in production', async () => {
  vi.stubEnv('DEV', false)
  try {
    const { wrapper } = await setup('/login')
    expect(wrapper.find('main button').exists()).toBe(false)
  } finally {
    vi.unstubAllEnvs()
  }
})

it.each(['/problems/create', '/problems/problem-a/edit/basic'])(
  'blocks development-only editing in production: %s',
  async (path) => {
    vi.stubEnv('DEV', false)
    setActivePinia(createPinia())
    useUserStore().$patch({ user: me, isInitialized: true })
    try {
      await appRouter.push(path)
      expect(appRouter.currentRoute.value.path).toBe('/')
    } finally {
      vi.unstubAllEnvs()
    }
  }
)

it.each([401, 503])(
  'keeps the real public contest visible before and after an authentication %i',
  async (status) => {
    const pinia = createPinia()
    setActivePinia(pinia)
    const pending = deferred<Response>()
    const contestResponse = {
      data: {
        id: '1',
        title: 'Public contest',
        description: '- Overview',
        visibility: 'public',
        startsAt: '2026-01-01T00:00:00Z',
        endsAt: '2026-01-01T01:00:00Z',
        participationMode: 'individual',
        rules: { tieBreak: 'icpc', penaltyMinutes: 5 },
        revision: '1'
      },
      response: new Response(null, { status: 200 })
    }
    const get = vi
      .fn<(request: Request) => Promise<Response>>()
      .mockImplementation((request) =>
        new URL(request.url).pathname === '/api/v1/me'
          ? pending.promise
          : Promise.resolve(Response.json(contestResponse.data))
      )
    vi.stubGlobal('fetch', get)
    try {
      await appRouter.push({ path: '/contest/1', force: true })
      const wrapper = mount(App, {
        global: {
          plugins: [pinia, appRouter, [VueQueryPlugin, { queryClient }]],
          stubs: { TopNavbar: true, BottomNavbar: true }
        }
      })
      wrappers.push(wrapper)
      await flushPromises()
      expect(wrapper.get('h1').text()).toBe('Public contest')
      expect(useUserStore().isInitialized).toBe(false)
      pending.resolve(new Response(null, { status }))
      await flushPromises()
      expect(wrapper.get('h1').text()).toBe('Public contest')
      expect(appRouter.currentRoute.value.path).toBe('/contest/1')
      expect(
        get.mock.calls.filter(([request]) => request.url.endsWith('/contests/1'))
      ).toHaveLength(1)
    } finally {
      await flushPromises()
    }
  }
)

it('does not mount protected content while the session is unknown and supports retry', async () => {
  const pinia = createPinia()
  setActivePinia(pinia)
  const getMe = vi.spyOn(apiClient, 'GET').mockRejectedValue(new Error('offline'))
  const stop = watchSessionRouting()
  const remove = appRouter.addRoute({
    path: '/private-test',
    meta: { requiresAuth: true },
    component: { template: '<p>Private content</p>' }
  })
  try {
    await appRouter.push('/private-test')
    const wrapper = mount(App, {
      global: {
        plugins: [pinia, appRouter, [VueQueryPlugin, { queryClient }]],
        stubs: { TopNavbar: true, BottomNavbar: true }
      }
    })
    wrappers.push(wrapper)
    expect(wrapper.text()).not.toContain('Private content')
    getMe.mockResolvedValue({ data: me, response: Response.json(me) })
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
