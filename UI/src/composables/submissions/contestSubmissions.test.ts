import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import type { JudgeRun, Submission } from '@/api/client'
import { useContestSubmissionsQuery } from '@/composables/submissions/useContestSubmissionsQuery'
import { useJudgeRunQuery } from '@/composables/submissions/useJudgeRunQuery'
import { cleanupQueries, deferred, me, mountQuery, setupSession } from '@/test/helpers'

vi.hoisted(() => vi.stubEnv('VITE_API_V1_BASE_URL', 'http://localhost/api/v1'))
const fetcher = vi.fn<typeof fetch>()
const item: Submission = {
  id: 'submission-a',
  userId: me.id,
  problemId: 'problem-a',
  languageId: 'cpp',
  context: { kind: 'contest', contestId: 'contest-a' },
  submittedAt: '2026-01-01T00:00:00Z',
  currentJudgeRunId: 'run-a'
}
const run: JudgeRun = {
  id: 'run-a',
  submissionId: item.id,
  problemVersionId: 'version-a',
  state: 'running',
  completedCaseCount: 2,
  totalCaseCount: 4,
  revision: 2,
  requestedAt: item.submittedAt,
  updatedAt: item.submittedAt,
  startedAt: item.submittedAt,
  finishedAt: null,
  result: null,
  failure: null
}
let context: ReturnType<typeof setupSession>
beforeEach(() => {
  context = setupSession()
  fetcher.mockReset()
  vi.stubGlobal('fetch', fetcher)
})
afterEach(() => {
  cleanupQueries()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

it('keeps the server cursor when a personal-history page has no matching contest and follows target changes', async () => {
  const conditions = ref({
    contestId: 'contest-b',
    scope: 'mine' as const,
    teamId: '',
    cursor: undefined as string | undefined,
    enabled: true
  })
  fetcher.mockResolvedValue(Response.json({ items: [item], nextCursor: 'opaque-next' }))
  const { result } = mountQuery(() => useContestSubmissionsQuery(conditions), context)
  await flushPromises()
  expect(result.view.value).toMatchObject({
    status: 'success',
    data: { items: [], nextCursor: 'opaque-next' }
  })
  conditions.value = { ...conditions.value, contestId: 'contest-a', cursor: 'opaque-next' }
  fetcher.mockResolvedValue(Response.json({ items: [item], nextCursor: null }))
  await flushPromises()
  expect(result.view.value).toMatchObject({
    status: 'success',
    data: { items: [item], nextCursor: null }
  })
  const request = fetcher.mock.calls.at(-1)?.[0] as Request
  expect(new URL(request.url).searchParams.get('cursor')).toBe('opaque-next')
  expect(new URL(request.url).pathname).toBe('/api/v1/me/submissions')
})

it('polls without rolling back a revision and stops at completion', async () => {
  vi.useFakeTimers()
  fetcher
    .mockResolvedValueOnce(Response.json(run))
    .mockResolvedValueOnce(Response.json({ ...run, revision: 1, completedCaseCount: 0 }))
    .mockResolvedValueOnce(
      Response.json({
        ...run,
        revision: 3,
        state: 'completed',
        finishedAt: item.submittedAt,
        result: { verdict: 'AC', score: 100, maxTimeMs: 1, maxMemoryBytes: 1024 }
      })
    )
  const { result } = mountQuery(
    () => useJudgeRunQuery({ submissionId: item.id, judgeRunId: run.id }, true),
    context
  )
  await flushPromises()
  await vi.advanceTimersByTimeAsync(2000)
  expect(result.view.value).toMatchObject({
    status: 'success',
    data: { revision: 2, completedCaseCount: 2 }
  })
  await vi.advanceTimersByTimeAsync(2000)
  expect(result.view.value).toMatchObject({
    status: 'success',
    data: { revision: 3, state: 'completed' }
  })
  await vi.advanceTimersByTimeAsync(10000)
  expect(fetcher).toHaveBeenCalledTimes(3)
})

it('discards a private result after logout and stops polling', async () => {
  vi.useFakeTimers()
  const pending = deferred<Response>()
  fetcher.mockReturnValue(pending.promise)
  const { result } = mountQuery(
    () => useJudgeRunQuery({ submissionId: item.id, judgeRunId: run.id }, true),
    context
  )
  await flushPromises()
  context.session.reset()
  pending.resolve(Response.json(run))
  await flushPromises()
  expect(result.view.value.status).toBe('pending')
  await vi.advanceTimersByTimeAsync(10000)
  expect(fetcher).toHaveBeenCalledTimes(1)
})
