import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter, RouterView } from 'vue-router'

import type { Contest } from '@/api/client'
import { queryClient } from '@/api/queryClient'
import appRouter from '@/router'
import { deferred, setupSession } from '@/test/helpers'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
vi.mock('@/components/editor/CodeEditor.vue', () => ({ default: { render: () => null } }))

const contest: Contest = {
  id: 'contest-a',
  title: 'Contest A',
  visibility: 'public',
  participationMode: 'individual',
  startsAt: '2020-01-01T00:00:00Z',
  endsAt: '2099-01-01T00:00:00Z',
  rules: { tieBreak: 'icpc', penaltyMinutes: 5 },
  revision: '1'
}
let wrapper: VueWrapper | undefined
afterEach(() => {
  wrapper?.unmount()
  queryClient.clear()
  vi.unstubAllGlobals()
})

const setup = async (fetcher: (request: Request) => Promise<Response>) => {
  const context = setupSession()
  vi.stubGlobal('fetch', fetcher)
  const router = createRouter({ history: createMemoryHistory(), routes: appRouter.options.routes })
  await router.push('/contest/contest-a/problems')
  wrapper = mount(RouterView, {
    global: {
      plugins: [context.pinia, [VueQueryPlugin, { queryClient }], router]
    }
  })
  await flushPromises()
  return { ...context, router, wrapper }
}

it('keeps the header mounted across tabs and shows the page heading before data arrives', async () => {
  const pending = deferred<Response>()
  const fetcher = vi.fn((request: Request) =>
    request.url.endsWith('/contests/contest-a')
      ? pending.promise
      : Promise.resolve(Response.json({ items: [], nextCursor: null }))
  )
  const { router, wrapper } = await setup(fetcher)
  const title = wrapper.get('h1').element
  const tabs = wrapper.get('nav[aria-label="コンテストのページ"]').element
  expect(wrapper.get('h2').text()).toBe('問題一覧')
  expect(wrapper.text()).not.toContain(contest.title)
  pending.resolve(Response.json(contest))
  await flushPromises()
  for (const tab of ['submit', 'submissions', 'clarifications', 'editorials', 'top', 'problems']) {
    await router.push({ name: `contest-${tab}`, params: { contestId: contest.id } })
    await flushPromises()
    expect(wrapper.get('h1').element).toBe(title)
    expect(wrapper.get('h1').text()).toBe(contest.title)
    expect(wrapper.get('nav[aria-label="コンテストのページ"]').element).toBe(tabs)
  }
  expect(
    fetcher.mock.calls.filter(([request]) => request.url.endsWith('/contests/contest-a'))
  ).toHaveLength(1)
})

it('retains content on refresh and transient failure, but removes it on permission loss', async () => {
  let response = Promise.resolve(Response.json(contest))
  const { wrapper } = await setup((request) =>
    request.url.endsWith('/contests/contest-a')
      ? response
      : Promise.resolve(Response.json({ items: [] }))
  )
  const title = wrapper.get('h1').element
  const heading = wrapper.get('h2').element
  const pending = deferred<Response>()
  response = pending.promise
  const refresh = queryClient.invalidateQueries({ queryKey: ['contest'], exact: false })
  await flushPromises()
  expect(wrapper.get('h1').element).toBe(title)
  expect(wrapper.get('h2').element).toBe(heading)
  pending.resolve(new Response(null, { status: 503 }))
  await refresh
  await flushPromises()
  expect(wrapper.get('h1').element).toBe(title)
  expect(wrapper.get('h2').element).toBe(heading)
  expect(wrapper.get('[role="alert"]').text()).toContain('読み込みに失敗')
  response = Promise.resolve(new Response(null, { status: 403 }))
  await wrapper.get('[role="alert"] button').trigger('click')
  await flushPromises()
  expect(wrapper.find('h1').exists()).toBe(false)
  expect(wrapper.find('h2').exists()).toBe(false)
  expect(wrapper.text()).toContain('閲覧する権限がありません')
})

it('clears the previous contest on navigation and refetches without credentials after logout', async () => {
  const nextContest = deferred<Response>()
  let response = nextContest.promise
  const fetcher = vi.fn((request: Request) => {
    if (request.url.endsWith('/contests/contest-a')) return Promise.resolve(Response.json(contest))
    if (request.url.endsWith('/contests/contest-b')) return response
    return Promise.resolve(Response.json({ items: [] }))
  })
  const { session, router, wrapper } = await setup(fetcher)
  await router.push('/contest/contest-b/problems')
  await flushPromises()
  expect(wrapper.text()).not.toContain(contest.title)
  expect(wrapper.get('h2').text()).toBe('問題一覧')
  nextContest.resolve(Response.json({ ...contest, id: 'contest-b', title: 'Contest B' }))
  await flushPromises()
  expect(wrapper.get('h1').text()).toBe('Contest B')
  const anonymous = deferred<Response>()
  response = anonymous.promise
  session.reset()
  await flushPromises()
  expect(wrapper.text()).not.toContain('Contest B')
  const requests = fetcher.mock.calls.filter(([request]) =>
    request.url.endsWith('/contests/contest-b')
  )
  expect(requests).toHaveLength(2)
  expect(requests.at(-1)?.[0].credentials).toBe('omit')
  anonymous.resolve(new Response(null, { status: 404 }))
  await flushPromises()
  expect(wrapper.text()).toContain('対象が見つかりません')
  expect(wrapper.text()).not.toContain('Contest B')
})
