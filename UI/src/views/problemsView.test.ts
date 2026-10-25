import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'

import { ProblemsApi, type ProblemSummaries } from '@/api/generated'
import { queryClient } from '@/api/queryClient'
import ProblemFilters from '@/components/problems/ProblemFilters.vue'
import { cleanupQueries, deferred, problem, setupSession, testRouter } from '@/test/helpers'
import ProblemsView from '@/views/ProblemsView.vue'

afterEach(cleanupQueries)
it('keeps the filter menu mounted when changing filters starts a page request', async () => {
  const context = setupSession()
  const router = testRouter()
  await router.push('/problems?page=3')
  const pending = deferred<ProblemSummaries>()
  vi.spyOn(ProblemsApi.prototype, 'getProblems')
    .mockResolvedValueOnce({ problems: [problem('a')], total: 60 })
    .mockReturnValueOnce(pending.promise)
  const wrapper = mount(ProblemsView, {
    global: { plugins: [context.pinia, [VueQueryPlugin, { queryClient }], router] }
  })
  await flushPromises()
  const filters = wrapper.getComponent(ProblemFilters)
  await filters.get('button').trigger('click')
  expect(filters.text()).toContain('以上')
  await filters.get('input[type="number"]').setValue(2)
  await flushPromises()
  expect(router.currentRoute.value.query.page).toBeUndefined()
  expect(wrapper.text()).toContain('読み込み中...')
  expect(wrapper.getComponent(ProblemFilters).element).toBe(filters.element)
  expect(filters.text()).toContain('以上')
  pending.resolve({ problems: [problem('b')], total: 2 })
  await flushPromises()
  expect(wrapper.text()).toContain('Problem b')
  wrapper.unmount()
})
