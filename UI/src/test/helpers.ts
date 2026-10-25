import { VueQueryPlugin } from '@tanstack/vue-query'
import { mount, type VueWrapper } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import type { Me, Problem, SubmissionSummary } from '@/api/generated'
import { queryClient } from '@/api/queryClient'
import { useUserStore } from '@/stores/user'

export const me: Me = {
  id: 'user-a',
  name: 'Alice',
  role: 'CommonUser',
  createdAt: new Date(0),
  updatedAt: new Date(0),
  authentication: {}
}
export const problem = (id: string): Problem => ({
  id,
  title: `Problem ${id}`,
  authorId: 'author',
  isPublic: true,
  difficulty: 3,
  statement: `Statement ${id}`,
  timeLimit: 1000,
  memoryLimit: 1024,
  solvedCount: 0,
  testcases: [],
  createdAt: new Date(0),
  updatedAt: new Date(0)
})
export const submission = (id: string): SubmissionSummary & { sourceCode: string } => ({
  id,
  title: `Problem ${id}`,
  userId: me.id,
  userName: me.name,
  problemId: id,
  submittedAt: new Date(0),
  languageId: 'cpp',
  totalScore: 0,
  maxTime: 0,
  maxMemory: 0,
  sourceCode: 'source code',
  codeLength: 10,
  judgeStatus: 'WJ'
})
export const deferred = <T>() => {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}
export const setupSession = () => {
  queryClient.clear()
  const pinia = createPinia()
  setActivePinia(pinia)
  const session = useUserStore()
  session.$patch({ user: me, isInitialized: true })
  return { pinia, session }
}
export const testRouter = () =>
  createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/problems/:id', name: 'problem-home', component: { render: () => null } },
      {
        path: '/problems/:id/submissions',
        name: 'problem-submissions',
        component: { render: () => null }
      },
      { path: '/users/:id', name: 'user-home', component: { render: () => null } },
      { path: '/:pathMatch(.*)*', component: { render: () => null } }
    ]
  })
const wrappers: VueWrapper[] = []
export const mountQuery = <T>(
  setup: () => T,
  context: ReturnType<typeof setupSession>,
  router = testRouter()
) => {
  let result!: T
  const wrapper = mount(
    defineComponent({
      setup: () => {
        result = setup()
        return () => h('div')
      }
    }),
    {
      global: { plugins: [context.pinia, [VueQueryPlugin, { queryClient }], router] }
    }
  )
  wrappers.push(wrapper)
  return { result, wrapper, router }
}
export const cleanupQueries = () => {
  wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
  queryClient.clear()
}
