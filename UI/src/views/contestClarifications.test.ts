import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import type { Clarification, Contest } from '@/api/client'
import { queryClient } from '@/api/queryClient'
import ClarificationCard from '@/components/contests/ClarificationCard.vue'
import { me, setupSession, testRouter } from '@/test/helpers'
import ContestClarificationsView from '@/views/ContestClarificationsView.vue'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
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
const question: Clarification = {
  id: 'question-a',
  authorId: 'user-b',
  body: '入力について',
  problemId: 'problem-a',
  answer: null,
  answeredBy: null,
  isPublic: false,
  revision: '1',
  createdAt: '2026-01-01T00:00:00Z'
}
let context: ReturnType<typeof setupSession>
const wrappers: VueWrapper[] = []
const fetcher = vi.fn<(request: Request) => Promise<Response>>()
const button = (wrapper: VueWrapper, name: string) =>
  wrapper.findAll('button').find((item) => item.text() === name)!
beforeEach(() => {
  context = setupSession()
  context.session.user = {
    ...me,
    roles: (['contestant', 'contest-setter'] as const).map((role) => ({
      role,
      scope: { kind: 'contest', contestId: contest.id },
      userId: me.id,
      grantedAt: '2026-01-01T00:00:00Z'
    }))
  }
  fetcher.mockReset()
  vi.stubGlobal('fetch', fetcher)
})
afterEach(() => {
  wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
  queryClient.clear()
  document.body.innerHTML = ''
  vi.unstubAllGlobals()
})

it('keeps question drafts through closing and failure, then closes and refreshes after sending', async () => {
  let fail = true
  const questions = [question]
  fetcher.mockImplementation(async (request) => {
    const path = new URL(request.url).pathname
    if (request.method === 'POST' && fail) return new Response(null, { status: 503 })
    if (request.method === 'POST') {
      const body = (await request.json()) as { body: string }
      const saved = { ...question, id: 'new-question', body: body.body, authorId: me.id }
      questions.unshift(saved)
      return Response.json(saved)
    }
    if (path.endsWith('/clarifications'))
      return Response.json({ items: questions, nextCursor: null })
    if (path.endsWith('/problems')) return Response.json({ items: [] })
    if (path.includes('/users/')) return new Response(null, { status: 503 })
    return Response.json(contest)
  })
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/contest/:contestId/clarifications', component: { render: () => null } },
      { path: '/users/:id', name: 'user-home', component: { render: () => null } }
    ]
  })
  await router.push('/contest/contest-a/clarifications')
  // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- ESLintでは、.vueからimportしたコンポーネントの型を解決できないため。
  const wrapper: VueWrapper = mount(ContestClarificationsView, {
    props: { contest: undefined },
    attachTo: document.body,
    global: {
      plugins: [context.pinia, [VueQueryPlugin, { queryClient }], router]
    }
  })
  wrappers.push(wrapper)
  await flushPromises()
  expect(wrapper.get('h2').text()).toBe('質問と回答')
  expect(wrapper.find('article').exists()).toBe(false)
  expect(button(wrapper, '質問する')).toBeUndefined()
  await wrapper.setProps({ contest })
  await flushPromises()
  expect(wrapper.find('textarea').exists()).toBe(false)
  expect(wrapper.text()).toContain('入力について')
  expect(wrapper.text()).toContain('不明なユーザー')
  await button(wrapper, '質問する').trigger('click')
  expect(document.activeElement).toBe(wrapper.get('textarea').element)
  await wrapper.get('textarea').setValue('送信する質問')
  await button(wrapper, '閉じる').trigger('click')
  expect(document.activeElement).toBe(button(wrapper, '質問する').element)
  await button(wrapper, '質問する').trigger('click')
  expect(wrapper.get<HTMLTextAreaElement>('textarea').element.value).toBe('送信する質問')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.get<HTMLTextAreaElement>('textarea').element.value).toBe('送信する質問')
  expect(wrapper.get('[role="alert"]').text()).toContain('質問を送信できませんでした')
  fail = false
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.find('textarea').exists()).toBe(false)
  expect(wrapper.findAll('article')).toHaveLength(2)
  expect(wrapper.text()).toContain('送信する質問')
  expect(wrapper.get(`a[href="/users/${me.id}"]`).text()).toBe('あなたの質問')
  expect(document.activeElement).toBe(button(wrapper, '質問する').element)
})

it('keeps an answer and its visibility when closed or saving fails', async () => {
  let fail = true
  const answers: unknown[] = []
  fetcher.mockImplementation(async (request) => {
    if (request.url.includes('/users/'))
      return Response.json({ ...me, id: 'user-b', displayName: 'Bob' })
    if (request.method === 'PUT') {
      const input = (await request.json()) as { body: string; isPublic: boolean }
      answers.push(input)
      return fail
        ? new Response(null, { status: 503 })
        : Response.json({ ...question, answer: input.body, isPublic: true })
    }
    return Response.json(question, { headers: { ETag: '"1"' } })
  })
  // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- ESLintでは、.vueからimportしたコンポーネントの型を解決できないため。
  const wrapper: VueWrapper = mount(ClarificationCard, {
    attachTo: document.body,
    props: {
      contestId: contest.id,
      question,
      canAnswer: true,
      problemLabel: 'A'
    },
    global: { plugins: [context.pinia, [VueQueryPlugin, { queryClient }], testRouter()] }
  })
  wrappers.push(wrapper)
  await flushPromises()
  expect(wrapper.get('a[href="/users/user-b"]').text()).toBe('Bob')
  await button(wrapper, '回答する').trigger('click')
  expect(document.activeElement).toBe(wrapper.get('textarea').element)
  await wrapper.get('textarea').setValue('負数も含みます。')
  await wrapper.get('input').setValue(true)
  expect(wrapper.text()).toContain('他のコンテストの参加者にも公開')
  await button(wrapper, '閉じる').trigger('click')
  expect(document.activeElement).toBe(button(wrapper, '回答する').element)
  await button(wrapper, '回答する').trigger('click')
  expect(wrapper.get<HTMLTextAreaElement>('textarea').element.value).toBe('負数も含みます。')
  expect(wrapper.get<HTMLInputElement>('input').element.checked).toBe(true)
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.get<HTMLTextAreaElement>('textarea').element.value).toBe('負数も含みます。')
  expect(wrapper.text()).toContain('回答を保存できませんでした')
  fail = false
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.find('form').exists()).toBe(false)
  expect(answers).toEqual([
    { body: '負数も含みます。', isPublic: true },
    { body: '負数も含みます。', isPublic: true }
  ])
})
