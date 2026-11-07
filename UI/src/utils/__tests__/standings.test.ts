import { describe, expect, it } from 'vitest'
import type { Contest } from '@/api/generated/models/Contest'
import type { StandingRow } from '@/types/standings'
import { canViewContestStandings } from '@/utils/contestStandingsAccess'
import {
  findStandingRowForUser,
  formatAcceptedElapsedTime,
  getLastAcceptedAt,
  mergeStandingProblemScores,
  sortStandingRows
} from '@/utils/standings'

const contest: Contest = {
  id: 'contest-1',
  title: '確認用コンテスト',
  visibility: 'public',
  participationMode: 'individual',
  startsAt: new Date('2026-10-01T00:00:00Z'),
  endsAt: new Date('2026-10-01T02:00:00Z'),
  rules: { tieBreak: 'atcoder', penaltyMinutes: 5 },
  revision: '1'
}

const rows: StandingRow[] = [
  { id: 'a', name: 'アオイ', rank: 1, score: 300, results: { c: { score: 100 } } },
  { id: 'b', name: 'ハル', rank: 2, score: 200, results: {} },
  { id: 'c', name: 'ミオ', rank: 3, score: 200, results: { c: { score: 100 } } }
]

describe('順位表の表示順', () => {
  it('正式APIの問題別得点と仮の提出情報を統合する', () => {
    const results = mergeStandingProblemScores(
      [
        { problemId: 'a', score: 100 },
        { problemId: 'b', score: 0 },
        { problemId: 'c', score: 0 }
      ],
      {
        a: { acceptedAt: '2026-10-01T00:12:05Z' },
        b: { pendingCount: 1 }
      }
    )
    expect(results).toEqual({
      a: { score: 100, acceptedAt: '2026-10-01T00:12:05Z' },
      b: { score: 0, pendingCount: 1 },
      c: { score: null }
    })
  })

  it('問題別に並べ替えても公式順位を保持する', () => {
    const sorted = sortStandingRows(rows, { column: 'c', direction: 'desc' })
    expect(sorted.map(({ id, rank }) => [id, rank])).toEqual([
      ['a', 1],
      ['c', 3],
      ['b', 2]
    ])
    expect(rows.map(({ id }) => id)).toEqual(['a', 'b', 'c'])
  })

  it('未回答の人は昇順で先頭に置く', () => {
    const sorted = sortStandingRows(rows, { column: 'c', direction: 'asc' })
    expect(sorted.map(({ id }) => id)).toEqual(['b', 'a', 'c'])
  })

  it('得点がない行は昇順で先頭，降順で末尾に置く', () => {
    const sparseRows: StandingRow[] = [
      { id: 'a', name: 'アオイ', rank: 1, score: 100, results: { b: { score: 100 } } },
      { id: 'b', name: 'ハル', rank: 2, score: 0, results: {} },
      { id: 'c', name: 'ミオ', rank: 3, score: 0, results: { b: { score: null } } },
      { id: 'd', name: 'レン', rank: 4, score: 0, results: { b: { score: 0 } } }
    ]
    expect(
      sortStandingRows(sparseRows, { column: 'b', direction: 'asc' }).map(({ id }) => id)
    ).toEqual(['b', 'c', 'd', 'a'])
    expect(
      sortStandingRows(sparseRows, { column: 'b', direction: 'desc' }).map(({ id }) => id)
    ).toEqual(['a', 'd', 'b', 'c'])
  })
})

describe('First Solverから順位表への移動', () => {
  it('チーム戦では提出者が所属するチーム行を選ぶ', () => {
    const team: StandingRow = {
      id: 'team-red',
      name: '赤チーム',
      rank: 1,
      score: 100,
      memberIds: ['u-aoi', 'u-haru'],
      results: { a: { score: 100 } }
    }
    expect(findStandingRowForUser([team], 'u-haru')).toBe(team)
    expect(findStandingRowForUser([team], 'unknown')).toBeUndefined()
  })
})

describe('AC時刻の表示', () => {
  it('コンテスト開始からの経過時間を得点の下に表示できる形にする', () => {
    expect(formatAcceptedElapsedTime('2026-10-01T00:12:05Z', contest.startsAt)).toBe('12:05')
    expect(formatAcceptedElapsedTime('2026-10-01T01:20:03Z', contest.startsAt)).toBe('1:20:03')
    expect(formatAcceptedElapsedTime('2026-09-30T23:59:59Z', contest.startsAt)).toBeNull()
  })

  it('問題ごとのAC時刻から最後のACを選ぶ', () => {
    expect(
      getLastAcceptedAt({
        a: { score: 100, acceptedAt: '2026-10-01T00:12:05Z' },
        b: { score: 100, acceptedAt: '2026-10-01T01:20:03Z' },
        c: { score: null }
      })
    ).toBe('2026-10-01T01:20:03Z')
  })
})

describe('順位表の閲覧条件', () => {
  const contestant = [{ role: 'contestant', scope: { kind: 'contest', contestId: 'contest-1' } }]

  it('開催中の公開大会は未ログインでも通常順位表を見られる', () => {
    const during = new Date('2026-10-01T01:00:00Z').getTime()
    expect(canViewContestStandings(contest, 'individual', during, [])).toBe(true)
    expect(canViewContestStandings(contest, 'team', during, [])).toBe(true)
    expect(canViewContestStandings(contest, 'individual', during, contestant)).toBe(true)
    expect(canViewContestStandings(contest, 'extended', during, contestant)).toBe(false)
  })

  it('開催中の非公開大会は参加者だけが通常順位表を見られる', () => {
    const privateContest = { ...contest, visibility: 'private' as const }
    const during = new Date('2026-10-01T01:00:00Z').getTime()
    expect(canViewContestStandings(privateContest, 'individual', during, [])).toBe(false)
    expect(canViewContestStandings(privateContest, 'individual', during, contestant)).toBe(true)
  })

  it('終了後の公開大会は匿名で閲覧できる', () => {
    const after = contest.endsAt.getTime()
    expect(canViewContestStandings(contest, 'individual', after, [])).toBe(true)
    expect(canViewContestStandings(contest, 'team', after, [])).toBe(true)
    expect(canViewContestStandings(contest, 'extended', after, [])).toBe(true)
  })

  it('終了後も非公開大会は参加者以外に見せない', () => {
    const privateContest = { ...contest, visibility: 'private' as const }
    const after = contest.endsAt.getTime()
    expect(canViewContestStandings(privateContest, 'individual', after, [])).toBe(false)
    expect(canViewContestStandings(privateContest, 'extended', after, contestant)).toBe(true)
  })
})
