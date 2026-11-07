import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, reactive, type App, type Component } from 'vue'
import { Configuration, DefaultConfig } from '@/api/generated'
import ProblemSubmission from '@/views/problem/ProblemSubmission.vue'
import SubmissionsTable from '@/components/SubmissionsTable.vue'

const submissionId = '00000000-0000-4000-8000-000000000001'
const problemId = '00000000-0000-4000-8000-000000000002'
const runId = '00000000-0000-4000-8000-000000000003'
const date = '2026-11-07T00:00:00Z'
const route = reactive({ params: { submissionId } })
vi.mock('vue-router', () => ({ useRoute: () => route }))

const submission = {
  id: submissionId,
  userId: submissionId,
  problemId,
  languageId: problemId,
  context: { kind: 'practice' },
  submittedAt: date,
  currentJudgeRunId: runId
}
const baseRun = {
  id: runId,
  submissionId,
  problemVersionId: problemId,
  state: 'queued',
  completedCaseCount: 0,
  totalCaseCount: null as number | null,
  revision: 0,
  updatedAt: date,
  requestedAt: date,
  startedAt: null,
  finishedAt: null as string | null,
  result: null as {
    verdict: string
    score: number
    maxTimeMs: number
    maxMemoryBytes: number
  } | null,
  failure: null as { code: string; message: string; requestId: string } | null
}
const completed = {
  ...baseRun,
  state: 'completed',
  completedCaseCount: 3,
  totalCaseCount: 3,
  revision: 6,
  finishedAt: date,
  result: { verdict: 'WA', score: 50, maxTimeMs: 12, maxMemoryBytes: 1024 }
}
const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status })
let app: App | undefined
let host: HTMLDivElement
let currentRun = { ...baseRun }
let currentRunId = runId
let failNext = false
let pending: (() => Promise<Response>) | undefined
async function judgeResponse(): Promise<Response> {
  if (pending) return pending()
  if (failNext) {
    failNext = false
    return json({ code: 'unavailable', message: '通信エラー', requestId: problemId }, 503)
  }
  return json(currentRun)
}
const fetchMock = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(async (url) => {
  if (url.includes('/judge-runs/')) return judgeResponse()
  if (url.includes('/me/submissions')) return json({ items: [submission], nextCursor: null })
  if (url.includes('/submissions/')) return json({ ...submission, currentJudgeRunId: currentRunId })
  if (url.includes('/problems/'))
    return json({ id: problemId, title: 'Sample', createdAt: date, updatedAt: date })
  throw new Error(`Unexpected request: ${url}`)
})

beforeEach(() => {
  vi.useFakeTimers()
  currentRun = { ...baseRun }
  currentRunId = runId
  failNext = false
  pending = undefined
  route.params.submissionId = submissionId
  DefaultConfig.config = new Configuration({ basePath: '/api/v1' })
  vi.stubGlobal('fetch', fetchMock)
  host = document.createElement('div')
  document.body.append(host)
})
afterEach(() => {
  app?.unmount()
  app = undefined
  host.remove()
  vi.useRealTimers()
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})
async function mount(component: Component) {
  app = createApp(component)
  app.mount(host)
  await vi.advanceTimersByTimeAsync(0)
  await nextTick()
}
async function poll() {
  await vi.advanceTimersByTimeAsync(2000)
  await nextTick()
}
const content = () => host.textContent?.replaceAll(/\s+/g, ' ')

describe.each([
  { name: 'submission detail', component: ProblemSubmission },
  { name: 'submission list', component: SubmissionsTable }
])('$name polling', ({ component }) => {
  it('shows partial counts, ignores older revisions and stops only after a final result', async () => {
    await mount(component)
    expect(content()).toContain('採点待ち')
    expect(content()).toContain('0 / 総数不明')
    currentRun = {
      ...baseRun,
      state: 'running',
      completedCaseCount: 1,
      totalCaseCount: 3,
      revision: 2
    }
    await poll()
    expect(content()).toContain('1 / 3')
    currentRun = { ...completed, revision: 1 }
    await poll()
    expect(content()).toContain('1 / 3')
    expect(content()).not.toContain('WA')
    currentRun = {
      ...baseRun,
      state: 'running',
      completedCaseCount: 2,
      totalCaseCount: 3,
      revision: 3
    }
    await poll()
    expect(content()).toContain('2 / 3')
    currentRun = {
      ...baseRun,
      state: 'aggregating',
      completedCaseCount: 3,
      totalCaseCount: 3,
      revision: 5
    }
    await poll()
    expect(content()).toContain('結果集計中')
    expect(content()).toContain('3 / 3')
    expect(content()).not.toContain('WA')
    currentRun = completed
    await poll()
    expect(content()).toContain('WA')
    const calls = fetchMock.mock.calls.length
    await vi.advanceTimersByTimeAsync(10000)
    expect(fetchMock).toHaveBeenCalledTimes(calls)
  })

  it('keeps the last progress during a network error and resumes polling', async () => {
    currentRun = {
      ...baseRun,
      state: 'running',
      completedCaseCount: 1,
      totalCaseCount: 3,
      revision: 2
    }
    await mount(component)
    failNext = true
    await poll()
    expect(content()).toContain('1 / 3')
    expect(host.querySelector('[role="alert"]')?.textContent).toContain('再取得')
    currentRun = completed
    await poll()
    expect(content()).toContain('WA')
    expect(host.querySelector('[role="alert"]')).toBeNull()
  })

  it('accepts a lower revision for a different judge run', async () => {
    currentRun = {
      ...baseRun,
      state: 'running',
      completedCaseCount: 2,
      totalCaseCount: 3,
      revision: 5
    }
    await mount(component)
    currentRunId = '00000000-0000-4000-8000-000000000004'
    currentRun = { ...baseRun, id: currentRunId }
    await poll()
    expect(content()).toContain('採点待ち')
    expect(content()).toContain('0 / 総数不明')
    expect(fetchMock.mock.calls.at(-1)?.[0]).toContain(`/judge-runs/${currentRunId}`)
  })

  it('stops on judge infrastructure failure without treating it as a verdict', async () => {
    await mount(component)
    currentRun = {
      ...baseRun,
      state: 'failed',
      revision: 1,
      finishedAt: date,
      failure: { code: 'judge_failed', message: '採点に失敗しました．', requestId: problemId }
    }
    await poll()
    expect(content()).toContain('採点失敗')
    expect(content()).not.toContain('AC')
    const calls = fetchMock.mock.calls.length
    await vi.advanceTimersByTimeAsync(10000)
    expect(fetchMock).toHaveBeenCalledTimes(calls)
  })

  it('aborts an in-flight request and leaves no polling timer after unmount', async () => {
    await mount(component)
    let resolve!: (response: Response) => void
    pending = () =>
      new Promise<Response>((done) => {
        resolve = done
      })
    await poll()
    const signal = fetchMock.mock.calls.at(-1)?.[1]?.signal
    app?.unmount()
    app = undefined
    expect(signal?.aborted).toBe(true)
    resolve(json(completed))
    await vi.advanceTimersByTimeAsync(10000)
    expect(vi.getTimerCount()).toBe(0)
  })
})

it('discards the old detail response when navigating to another submission', async () => {
  await mount(ProblemSubmission)
  let resolve!: (response: Response) => void
  pending = () =>
    new Promise<Response>((done) => {
      resolve = done
    })
  await poll()
  const signal = fetchMock.mock.calls.at(-1)?.[1]?.signal
  pending = undefined
  route.params.submissionId = problemId
  await vi.advanceTimersByTimeAsync(0)
  expect(signal?.aborted).toBe(true)
  resolve(json(completed))
  await vi.advanceTimersByTimeAsync(0)
  expect(content()).toContain('採点待ち')
  expect(content()).not.toContain('WA')
})
