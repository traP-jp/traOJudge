import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { LanguageApi, SubmissionsApi } from '@/api/generated'
import PrimaryButton from '@/components/Controls/PrimaryButton.vue'
import JudgeResultBadge from '@/components/JudgeResultBadge.vue'
import NumberedPagination from '@/components/NumberedPagination.vue'
import QueryState from '@/components/QueryState.vue'
import SubmissionsTable from '@/components/SubmissionsTable.vue'
import SubmitForm from '@/components/SubmitForm.vue'
import { cleanupQueries, deferred, setupSession, submission, testRouter } from '@/test/helpers'

vi.mock('@/components/CodeBlock.vue', () => ({
  default: defineComponent({
    props: { modelValue: { type: String, default: '' } },
    emits: ['update:modelValue'],
    setup:
      (_, { emit }) =>
      () =>
        h('button', { onClick: () => emit('update:modelValue', 'source code') }, 'Fill code')
  })
}))
afterEach(cleanupQueries)

it('updates the same badge from WJ to AC', async () => {
  const wrapper = mount(JudgeResultBadge, { props: { status: 'WJ' } })
  const oldClasses = wrapper.classes()
  await wrapper.setProps({ status: 'AC' })
  expect(wrapper.text()).toBe('AC')
  expect(wrapper.classes()).not.toEqual(oldClasses)
  wrapper.unmount()
})
it('renders both selected columns and supplied links without fetching', () => {
  const wrapper = mount(SubmissionsTable, {
    props: {
      rows: [{ ...submission('a'), problemHref: '/custom/problem', userHref: '/custom/user' }],
      columns: ['title', 'userName']
    }
  })
  expect(wrapper.findAll('a').map((link) => link.attributes('href'))).toEqual([
    '/custom/problem',
    '/custom/user'
  ])
  wrapper.unmount()
})
it('normalizes an out of range page to 1, never 0', () => {
  const wrapper = mount(NumberedPagination, { props: { page: 3, totalPages: 1 } })
  expect(wrapper.emitted('normalize')).toEqual([[1]])
  expect(wrapper.find('button').exists()).toBe(false)
  wrapper.unmount()
})
it('does not render old content on a permission error', async () => {
  const wrapper = mount(QueryState, {
    props: { view: { status: 'success', data: 'secret', refreshing: false } },
    slots: { default: 'secret' }
  })
  await wrapper.setProps({ view: { status: 'error', message: '閲覧する権限がありません。' } })
  expect(wrapper.text()).not.toContain('secret')
  await wrapper.find('button').trigger('click')
  expect(wrapper.emitted('retry')).toHaveLength(1)
  wrapper.unmount()
})
it('does not navigate to A after switching the form to B during submission', async () => {
  const context = setupSession()
  const router = testRouter()
  await router.push('/problems/a')
  vi.spyOn(LanguageApi.prototype, 'getLanguages').mockResolvedValue([{ id: 'cpp', name: 'C++' }])
  const response = deferred<ReturnType<typeof submission>>()
  const post = vi
    .spyOn(SubmissionsApi.prototype, 'postSubmission')
    .mockReturnValue(response.promise)
  const wrapper = mount(SubmitForm, {
    props: { problemId: 'a' },
    global: { plugins: [context.pinia, router] }
  })
  await flushPromises()
  await wrapper
    .findAll('button')
    .find((button) => button.text() === 'Fill code')
    ?.trigger('click')
  await wrapper.getComponent(PrimaryButton).trigger('click')
  expect(post).toHaveBeenCalledTimes(1)
  await wrapper.setProps({ problemId: 'b' })
  await router.push('/problems/b')
  response.resolve(submission('a'))
  await flushPromises()
  expect(router.currentRoute.value.path).toBe('/problems/b')
  wrapper.unmount()
})
it('retains source on a failed submission and allows retry', async () => {
  const context = setupSession()
  const router = testRouter()
  await router.push('/problems/a')
  vi.spyOn(LanguageApi.prototype, 'getLanguages').mockResolvedValue([{ id: 'cpp', name: 'C++' }])
  const post = vi
    .spyOn(SubmissionsApi.prototype, 'postSubmission')
    .mockRejectedValueOnce(new Error('offline'))
    .mockResolvedValueOnce(submission('a'))
  vi.spyOn(window, 'alert').mockImplementation(() => undefined)
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
  const wrapper = mount(SubmitForm, {
    props: { problemId: 'a' },
    global: { plugins: [context.pinia, router] }
  })
  await flushPromises()
  await wrapper
    .findAll('button')
    .find((button) => button.text() === 'Fill code')
    ?.trigger('click')
  await wrapper.getComponent(PrimaryButton).trigger('click')
  await flushPromises()
  await wrapper.getComponent(PrimaryButton).trigger('click')
  await flushPromises()
  expect(post).toHaveBeenCalledTimes(2)
  expect(post.mock.calls[1][0]?.postSubmissionRequest?.source).toBe('source code')
  expect(router.currentRoute.value.path).toBe('/problems/a/submissions')
  wrapper.unmount()
})
