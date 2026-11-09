import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import type { Contest } from '@/api/client'
import { queryClient } from '@/api/queryClient'
import { setupSession } from '@/test/helpers'
import ContestProblemView from '@/views/ContestProblemView.vue'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
let wrapper: VueWrapper | undefined
afterEach(() => {
  wrapper?.unmount()
  queryClient.clear()
  document.body.innerHTML = ''
  vi.unstubAllGlobals()
})

it('switches problems, updates both submit links, and closes the picker on Escape and outside focus', async () => {
  const context = setupSession()
  const contest: Contest = {
    id: 'contest-a',
    title: 'Contest',
    visibility: 'public',
    participationMode: 'individual',
    startsAt: '2020-01-01T00:00:00Z',
    endsAt: '2099-01-01T00:00:00Z',
    rules: { tieBreak: 'icpc', penaltyMinutes: 5 },
    revision: '1'
  }
  const items = ['a', 'b'].map((id, order) => ({ problemId: id, label: id.toUpperCase(), order }))
  vi.stubGlobal(
    'fetch',
    vi.fn((request: Request) => {
      const path = new URL(request.url).pathname
      if (path.endsWith('/contest-a/problems')) return Promise.resolve(Response.json({ items }))
      if (path.includes('/problems/')) {
        const id = path.split('/').at(-1)
        return Promise.resolve(
          Response.json({ id, title: `問題${id}`, statement: `## ${id}の本文` })
        )
      }
      return Promise.resolve(Response.json(contest))
    })
  )
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      {
        path: '/contest/:contestId/problems/:problemId',
        name: 'contest-problem',
        component: { render: () => null }
      },
      {
        path: '/contest/:contestId/problems',
        name: 'contest-problems',
        component: { render: () => null }
      },
      {
        path: '/contest/:contestId/submit',
        name: 'contest-submit',
        component: { render: () => null }
      }
    ]
  })
  await router.push('/contest/contest-a/problems/a')

  wrapper = mount(ContestProblemView, {
    props: { contest },
    attachTo: document.body,
    global: {
      plugins: [context.pinia, [VueQueryPlugin, { queryClient }], router]
    }
  })
  await flushPromises()
  expect(wrapper.get('h2').text()).toBe('A — 問題a')
  const trigger = wrapper.get<HTMLButtonElement>('button[aria-label="問題を切り替える"]')
  await trigger.trigger('click')
  expect(wrapper.get('nav[aria-label="問題の切り替え"] a[aria-current="page"]').text()).toContain(
    'A — 問題a'
  )
  await wrapper.get('a[href="/contest/contest-a/problems/b"]').trigger('click')
  await flushPromises()
  expect(wrapper.get('h2').text()).toBe('B — 問題b')
  expect(wrapper.find('nav[aria-label="問題の切り替え"]').exists()).toBe(false)
  expect(wrapper.findAll('a[href="/contest/contest-a/submit?problemId=b"]')).toHaveLength(2)
  expect(document.activeElement).toBe(trigger.element)
  await trigger.trigger('click')
  expect(wrapper.get('nav a[aria-current="page"]').text()).toContain('B — 問題b')
  await trigger.trigger('keydown', { key: 'Escape' })
  expect(trigger.attributes('aria-expanded')).toBe('false')
  expect(document.activeElement).toBe(trigger.element)
  await trigger.trigger('click')
  wrapper.get<HTMLAnchorElement>('a[href="/contest/contest-a/problems"]').element.focus()
  await flushPromises()
  expect(trigger.attributes('aria-expanded')).toBe('false')
})
