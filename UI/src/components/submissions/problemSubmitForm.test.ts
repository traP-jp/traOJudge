import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { LanguageApi, SubmissionsApi } from '@/api/generated'
import PrimaryButton from '@/components/controls/PrimaryButton.vue'
import ProblemSubmitForm from '@/components/submissions/ProblemSubmitForm.vue'
import { cleanupQueries, deferred, setupSession, submission, testRouter } from '@/test/helpers'

vi.mock('@/components/editor/CodeEditor.vue', () => ({
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

it('does not navigate to A after switching the form to B during submission', async () => {
  const context = setupSession()
  const router = testRouter()
  await router.push('/problems/a')
  vi.spyOn(LanguageApi.prototype, 'getLanguages').mockResolvedValue([{ id: 'cpp', name: 'C++' }])
  const response = deferred<ReturnType<typeof submission>>()
  const post = vi
    .spyOn(SubmissionsApi.prototype, 'postSubmission')
    .mockReturnValue(response.promise)
  const wrapper = mount(ProblemSubmitForm, {
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
  const wrapper = mount(ProblemSubmitForm, {
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
