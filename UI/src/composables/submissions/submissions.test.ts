import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'

import type { SubmissionSummaries } from '@/api/generated'
import { SubmissionsApi } from '@/api/generated'
import { useSubmissionsQuery } from '@/composables/submissions/useSubmissionsQuery'
import { useSubmitSolution } from '@/composables/submissions/useSubmitSolution'
import { cleanupQueries, deferred, mountQuery, setupSession, submission } from '@/test/helpers'

let context: ReturnType<typeof setupSession>
beforeEach(() => {
  context = setupSession()
})
afterEach(cleanupQueries)

describe('submission lists and updates', () => {
  it('keeps page 3 when page 2 responds last and reacts to a new user', async () => {
    const page2 = deferred<SubmissionSummaries>()
    const get = vi
      .spyOn(SubmissionsApi.prototype, 'getSubmissions')
      .mockReturnValueOnce(page2.promise)
      .mockResolvedValue({ submissions: [submission('new')], total: 60 })
    const page = ref(2)
    const userId = ref('a')
    const { result } = mountQuery(
      () => useSubmissionsQuery(() => ({ kind: 'user', userId: userId.value }), page),
      context
    )
    page.value = 3
    await flushPromises()
    page2.resolve({ submissions: [submission('old')], total: 60 })
    await flushPromises()
    expect(result.view.value).toMatchObject({
      status: 'success',
      data: { rows: [{ id: 'new' }], totalPages: 3 }
    })
    expect(get.mock.calls[1][0]).toMatchObject({ offset: 40 })
    userId.value = 'b'
    await flushPromises()
    expect(get.mock.lastCall?.[0]?.userId).toBe('b')
  })

  it('invalidates the submitted problem and current user without refreshing unrelated lists', async () => {
    const get = vi
      .spyOn(SubmissionsApi.prototype, 'getSubmissions')
      .mockResolvedValue({ submissions: [], total: 0 })
    const post = vi
      .spyOn(SubmissionsApi.prototype, 'postSubmission')
      .mockResolvedValue(submission('a'))
    mountQuery(() => useSubmissionsQuery({ kind: 'problem', problemId: 'a' }, 1), context)
    mountQuery(
      () => useSubmissionsQuery({ kind: 'user', userId: context.session.userId }, 1),
      context
    )
    mountQuery(() => useSubmissionsQuery({ kind: 'problem', problemId: 'b' }, 1), context)
    await flushPromises()
    const { result } = mountQuery(useSubmitSolution, context)
    expect(await result.submit('a', { languageId: 'cpp', source: 'int main(){}' })).toBe(true)
    expect(post.mock.calls[0][0]).toMatchObject({ problemId: 'a' })
    expect(get).toHaveBeenCalledTimes(5)
    expect(get.mock.calls.filter(([request]) => request?.problemIdInQuery === 'b')).toHaveLength(1)
  })

  it('freezes the payload, prevents duplicate submission and discards completion after logout', async () => {
    const response = deferred<ReturnType<typeof submission>>()
    const post = vi
      .spyOn(SubmissionsApi.prototype, 'postSubmission')
      .mockReturnValue(response.promise)
    const { result } = mountQuery(useSubmitSolution, context)
    const payload = { languageId: 'cpp', source: 'old' }
    const pending = result.submit('a', payload)
    payload.source = 'new'
    expect(await result.submit('b', payload)).toBe(false)
    expect(post.mock.calls[0][0]?.postSubmissionRequest?.source).toBe('old')
    context.session.reset()
    await nextTick()
    response.resolve(submission('a'))
    expect(await pending).toBe(false)
    expect(result.isSubmitting.value).toBe(false)
  })
})
