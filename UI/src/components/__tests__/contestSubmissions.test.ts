import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, reactive, type App, type Component } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import ContestSubmissionsTable from '@/components/ContestSubmissionsTable.vue'
import ContestSubmissionsView from '@/views/ContestSubmissionsView.vue'
import { useUserStore } from '@/stores/user'
import type { ContestSubmissionScope } from '@/types/contestSubmissions'

const route = reactive({
  params: { contestId: 'contest' },
  query: {} as Record<string, string | undefined>,
  fullPath: '/contest/contest/submissions',
  name: 'contest-submissions'
})
const replace = vi.fn(async ({ query }: { query: typeof route.query }) => {
  route.query = query
})
vi.mock('vue-router', () => ({ useRoute: () => route, useRouter: () => ({ replace }) }))
const date = '2026-10-01T05:59:00Z'
const submission = (id: string, overrides = {}) => ({
  id,
  userId: 'other',
  problemId: 'a',
  languageId: 'cpp',
  context: { kind: 'contest', contestId: 'contest' },
  submittedAt: date,
  currentJudgeRunId: `run-${id}`,
  ...overrides
})
const run = (id: string, overrides = {}) => ({
  id: `run-${id}`,
  submissionId: id,
  problemVersionId: 'version',
  state: 'completed',
  completedCaseCount: 3,
  totalCaseCount: 3,
  revision: 6,
  requestedAt: date,
  updatedAt: date,
  startedAt: date,
  finishedAt: date,
  result: { verdict: 'WA', score: 50, maxTimeMs: 12, maxMemoryBytes: 1024 },
  failure: null,
  ...overrides
})
const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status })
const contest = {
  id: 'contest',
  title: '秋のコンテスト',
  visibility: 'public',
  description: '',
  participationMode: 'individual',
  startsAt: '2026-10-01T04:00:00Z',
  endsAt: '2026-10-01T06:00:00Z',
  rules: { tieBreak: 'atcoder', penaltyMinutes: 5 },
  revision: '1'
}
let pageItems: ReturnType<typeof submission>[]
let board: typeof contest
let roleItems: { role: string; scope: { kind: string; contestId: string } }[]
let respond:
  | ((url: URL, init?: RequestInit) => Response | Promise<Response> | undefined)
  | undefined
const fetchMock = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(
  async (url, init) => {
    const parsed = new URL(url, 'http://localhost')
    const custom = respond?.(parsed, init)
    if (custom) return custom
    const path = parsed.pathname
    if (path.includes('/judge-runs/'))
      return json(run(path.split('/submissions/')[1]!.split('/')[0]!))
    if (path.endsWith('/submissions')) return json({ items: pageItems, nextCursor: null })
    if (path.startsWith('/api/v1/submissions/')) return json(submission(path.split('/').at(-1)!))
    if (path.startsWith('/api/v1/users/'))
      return json({
        id: path.split('/').at(-1),
        displayName: `ユーザー ${path.split('/').at(-1)}`,
        revision: '1',
        bio: '',
        githubId: '',
        atcoderId: '',
        xId: ''
      })
    if (path === '/api/v1/contests/contest') return json(board)
    if (path === '/api/v1/contests/contest/problems')
      return json({
        items: [
          { problemId: 'a', label: 'A', order: 0 },
          { problemId: 'private', label: 'B', order: 1 }
        ]
      })
    if (path === '/api/v1/problems/a')
      return json({ id: 'a', title: '二数の和', createdAt: date, updatedAt: date })
    if (path === '/api/v1/problems/private') return json({ message: '非公開' }, 403)
    if (path === '/api/v1/languages')
      return json({ items: [{ id: 'cpp', name: 'C++', versionLabel: '14', tags: [] }] })
    if (path === '/api/v1/me/roles') return json({ items: roleItems })
    if (path.endsWith('/standings/team'))
      return json({
        items: [
          {
            teamId: 'own-team',
            teamName: '赤チーム',
            problemScores: [],
            members: [{ userId: 'me', displayName: '自分' }]
          },
          {
            teamId: 'other-team',
            teamName: '青チーム',
            problemScores: [],
            members: [{ userId: 'other', displayName: '他人' }]
          }
        ]
      })
    throw new Error(`Unexpected request: ${url}`)
  }
)
let app: App | undefined
let host: HTMLDivElement
let props: {
  contestId: string
  scope: ContestSubmissionScope
  viewerId: string
  teamId: string
  problemId: string
  problems: { problemId: string; label: string; order: number; title: string; canOpen: boolean }[]
  teams: { id: string; name: string; members: { userId: string; displayName: string }[] }[]
  languages: { id: string; name: string; versionLabel: string; tags: string[] }[]
  canViewAll: boolean
  teamContest: boolean
}
beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-11-08T00:00:00Z'))
  setActivePinia(createPinia())
  pageItems = [submission('first')]
  board = { ...contest }
  roleItems = []
  respond = undefined
  route.query = {}
  props = reactive({
    contestId: 'contest',
    scope: 'all',
    viewerId: 'me',
    teamId: 'own-team',
    problemId: '',
    problems: [{ problemId: 'a', label: 'A', order: 0, title: '二数の和', canOpen: true }],
    teams: [],
    languages: [{ id: 'cpp', name: 'C++', versionLabel: '14', tags: [] }],
    canViewAll: true,
    teamContest: false
  })
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
async function flush() {
  await vi.advanceTimersByTimeAsync(0)
  await nextTick()
}
async function mount(component: Component = ContestSubmissionsTable) {
  app = createApp({
    render: () => h(component, component === ContestSubmissionsTable ? props : {})
  })
  app.component('RouterLink', {
    setup:
      (_, { slots }) =>
      () =>
        h('a', slots.default?.())
  })
  app.mount(host)
  await flush()
}
const content = () => host.textContent?.replaceAll(/\s+/g, ' ')
const listCalls = () =>
  fetchMock.mock.calls.filter(([url]) =>
    new URL(url, 'http://localhost').pathname.endsWith('/submissions')
  )
function click(label: string) {
  const button = [...host.querySelectorAll('button')].find((button) =>
    button.textContent?.trim().startsWith(label)
  )
  expect(button, label).toBeTruthy()
  button!.click()
}
function login() {
  useUserStore().user = {
    id: 'me',
    displayName: '自分',
    bio: '',
    githubId: '',
    atcoderId: '',
    xId: '',
    revision: '1',
    roles: []
  }
}

describe('contest submissions', () => {
  it('lists public results and filters out practice and other contests', async () => {
    pageItems = [
      submission('first'),
      submission('practice', { context: { kind: 'practice' } }),
      submission('elsewhere', { context: { kind: 'contest', contestId: 'other-contest' } })
    ]
    await mount()
    expect(host.querySelectorAll('tbody tr')).toHaveLength(1)
    expect(content()).toContain('ユーザー other')
    expect(content()).toContain('C++')
    expect(content()).toContain('50')
    expect(content()).toContain('WA')
    expect(listCalls()[0]![0]).toContain('/contests/contest/submissions')
    expect(fetchMock.mock.calls.some(([url]) => url.includes('run-practice'))).toBe(false)
  })

  it('skips nonmatching cursor pages and resets pagination when the problem changes', async () => {
    props.problemId = 'b'
    respond = (url) => {
      if (!url.pathname.endsWith('/submissions')) return
      const cursor = url.searchParams.get('cursor')
      if (!cursor) return json({ items: [submission('a')], nextCursor: 'page2' })
      if (cursor === 'page2')
        return json({ items: [submission('b', { problemId: 'b' })], nextCursor: 'page3' })
      return json({ items: [submission('last', { problemId: 'b' })], nextCursor: null })
    }
    await mount()
    expect(listCalls()).toHaveLength(2)
    expect(host.querySelectorAll('tbody tr')).toHaveLength(1)
    click('次へ')
    await flush()
    expect(content()).toContain('2ページ')
    click('前へ')
    await flush()
    expect(content()).toContain('1ページ')
    props.problemId = 'a'
    await flush()
    expect(listCalls().at(-1)![0]).not.toContain('cursor=')
    expect(content()).toContain('二数の和')
  })

  it('uses the own-team endpoint before the end and the public list afterwards', async () => {
    props.scope = 'team'
    props.canViewAll = false
    pageItems = [
      submission('own', { teamId: 'own-team' }),
      submission('other', { teamId: 'other-team' })
    ]
    await mount()
    expect(listCalls()[0]![0]).toContain('/teams/own-team/submissions')
    expect(host.querySelectorAll('tbody tr')).toHaveLength(1)
    props.canViewAll = true
    await flush()
    expect(listCalls().at(-1)![0]).toContain('/contests/contest/submissions')
    expect(listCalls().at(-1)![0]).not.toContain('/teams/')
    expect(host.querySelectorAll('tbody tr')).toHaveLength(1)
  })

  it('uses the current user list for mine and excludes other users', async () => {
    props.scope = 'mine'
    pageItems = [submission('own', { userId: 'me' }), submission('other')]
    await mount()
    expect(listCalls()[0]![0]).toContain('/me/submissions')
    expect(host.querySelectorAll('tbody tr')).toHaveLength(1)
    expect(content()).toContain('ユーザー me')
    expect(content()).not.toContain('ユーザー other')
  })

  it('polls pending runs, rejects older revisions, follows rejudges, and stops on completion', async () => {
    let current = run('first', {
      state: 'running',
      revision: 2,
      completedCaseCount: 1,
      result: null,
      finishedAt: null
    })
    let currentId = current.id
    respond = (url) => {
      if (url.pathname.includes('/judge-runs/')) return json(current)
      if (url.pathname === '/api/v1/submissions/first')
        return json(submission('first', { currentJudgeRunId: currentId }))
    }
    await mount()
    expect(content()).toContain('WJ 1/3...')
    current = run('first', { revision: 1 })
    await vi.advanceTimersByTimeAsync(2000)
    expect(content()).toContain('WJ 1/3...')
    current = run('first', {
      id: 'new-run',
      revision: 0,
      state: 'queued',
      completedCaseCount: 0,
      totalCaseCount: null,
      result: null
    })
    currentId = 'new-run'
    await vi.advanceTimersByTimeAsync(2000)
    expect(content()).toContain('WJ 0/?...')
    expect(fetchMock.mock.calls.at(-1)![0]).toContain('/judge-runs/new-run')
    current = run('first', { id: 'new-run' })
    await vi.advanceTimersByTimeAsync(2000)
    expect(content()).toContain('WA')
    expect(content()).not.toContain('ケース完了')
    const calls = fetchMock.mock.calls.length
    await vi.advanceTimersByTimeAsync(8000)
    expect(fetchMock).toHaveBeenCalledTimes(calls)
  })

  it('retains rows on result errors, retries transient errors, and stops retrying forbidden results', async () => {
    let status = 503
    respond = (url) =>
      url.pathname.includes('/judge-runs/') ? json({ message: 'unavailable' }, status) : undefined
    await mount()
    expect(content()).toContain('取得失敗')
    expect(host.querySelectorAll('tbody tr')).toHaveLength(1)
    await vi.advanceTimersByTimeAsync(2000)
    status = 403
    await vi.advanceTimersByTimeAsync(2000)
    expect(content()).toContain('閲覧不可')
    const calls = fetchMock.mock.calls.length
    await vi.advanceTimersByTimeAsync(8000)
    expect(fetchMock).toHaveBeenCalledTimes(calls)
    respond = undefined
    click('更新')
    await flush()
    expect(content()).toContain('WA')
  })

  it('aborts polling on a filter change and ignores late responses', async () => {
    let finish: ((value: Response) => void) | undefined
    let signal: AbortSignal | null | undefined
    let requests = 0
    respond = (url, init) => {
      if (!url.pathname.includes('/judge-runs/')) return
      requests++
      if (requests === 1)
        return json(run('first', { state: 'queued', result: null, completedCaseCount: 0 }))
      signal = init?.signal
      return new Promise<Response>((resolve) => {
        finish = resolve
      })
    }
    await mount()
    await vi.advanceTimersByTimeAsync(2000)
    expect(finish).toBeTypeOf('function')
    props.problemId = 'missing'
    await flush()
    expect(signal?.aborted).toBe(true)
    finish!(json(run('first')))
    await flush()
    expect(content()).toContain('条件に一致する提出はありません')
    expect(content()).not.toContain('WA')
    app!.unmount()
    app = undefined
    const calls = fetchMock.mock.calls.length
    await vi.advanceTimersByTimeAsync(8000)
    expect(fetchMock).toHaveBeenCalledTimes(calls)
  })

  it('shows an error for repeated cursors instead of requesting indefinitely', async () => {
    props.problemId = 'missing'
    respond = (url) =>
      url.pathname.endsWith('/submissions') ? json({ items: [], nextCursor: 'loop' }) : undefined
    await mount()
    expect(host.querySelector('[role="alert"]')?.textContent).toContain('続きを取得できません')
    expect(listCalls()).toHaveLength(2)
  })
})

describe('contest submissions page access and URL filters', () => {
  it('allows anonymous viewers after the end even when the problem is private', async () => {
    route.query = { problemId: 'private' }
    pageItems = [submission('private-submission', { problemId: 'private' })]
    await mount(ContestSubmissionsView)
    expect(content()).toContain('問題 B')
    expect(content()).toContain('WA')
    expect(content()).toContain('50')
    expect(listCalls()).toHaveLength(1)
    expect(host.querySelector<HTMLSelectElement>('#submission-problem')?.value).toBe('private')
    expect(fetchMock.mock.calls.some(([url]) => url.endsWith('/me/roles'))).toBe(false)
    click('自分の提出')
    await flush()
    expect(route.query.problemId).toBe('private')
    expect(route.query.scope).toBe('mine')
    expect(content()).toContain('ログインしてください')
    expect(listCalls()).toHaveLength(1)
  })

  it('defaults to mine during the contest and blocks the all query', async () => {
    login()
    board.endsAt = '2027-01-01T00:00:00Z'
    await mount(ContestSubmissionsView)
    expect(listCalls()[0]![0]).toContain('/me/submissions')
    click('すべて')
    await flush()
    expect(content()).toContain('コンテスト終了後に公開されます')
    expect(listCalls()).toHaveLength(1)
  })

  it('grants setters access only for their assigned contest', async () => {
    login()
    board.endsAt = '2027-01-01T00:00:00Z'
    roleItems = [{ role: 'contest-setter', scope: { kind: 'contest', contestId: 'contest' } }]
    await mount(ContestSubmissionsView)
    expect(listCalls()[0]![0]).toContain('/contests/contest/submissions')
    app!.unmount()
    app = undefined
    fetchMock.mockClear()
    roleItems[0]!.scope.contestId = 'elsewhere'
    await mount(ContestSubmissionsView)
    expect(listCalls()[0]![0]).toContain('/me/submissions')
  })

  it('limits team choices before the end and uses the selected team after the end', async () => {
    login()
    board.participationMode = 'team'
    board.endsAt = '2027-01-01T00:00:00Z'
    route.query = { scope: 'team' }
    pageItems = [submission('own', { teamId: 'own-team' })]
    await mount(ContestSubmissionsView)
    expect(host.querySelectorAll('#submission-team option')).toHaveLength(1)
    expect(listCalls()[0]![0]).toContain('/teams/own-team/submissions')
    route.query.teamId = 'other-team'
    await flush()
    expect(content()).toContain('このチームの提出は表示できません')
    expect(listCalls()).toHaveLength(1)
    app!.unmount()
    app = undefined
    board.endsAt = contest.endsAt
    await mount(ContestSubmissionsView)
    expect(host.querySelectorAll('#submission-team option')).toHaveLength(2)
    expect(listCalls().at(-1)![0]).toContain('/contests/contest/submissions')
  })

  it('switches to public access at the contest end without reloading the browser', async () => {
    board.endsAt = new Date(Date.now() + 1000).toISOString()
    await mount(ContestSubmissionsView)
    expect(listCalls()).toHaveLength(0)
    expect(content()).toContain('コンテスト終了後に公開されます')
    await vi.advanceTimersByTimeAsync(1100)
    expect(listCalls()).toHaveLength(1)
    expect(content()).toContain('WA')
  })

  it('does not request submissions for an unknown problem', async () => {
    route.query = { problemId: 'unknown' }
    await mount(ContestSubmissionsView)
    expect(content()).toContain('このコンテストに登録されていません')
    expect(listCalls()).toHaveLength(0)
  })
})
