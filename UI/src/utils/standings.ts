import type { ProblemResult, StandingResultDetails, StandingRow } from '@/types/standings'

export type StandingsSort = {
  column: 'total' | string
  direction: 'asc' | 'desc'
}

export type StandingSelection = {
  userId: string
  problemId: string
}

export function findStandingRowForUser(
  rows: StandingRow[],
  userId: string
): StandingRow | undefined {
  return rows.find((row) => row.id === userId || row.memberIds?.includes(userId))
}

export function sortStandingRows(rows: StandingRow[], sort: StandingsSort): StandingRow[] {
  const factor = sort.direction === 'desc' ? -1 : 1
  return [...rows].sort((left, right) => {
    const leftScore = sort.column === 'total' ? left.score : left.results[sort.column]?.score
    const rightScore = sort.column === 'total' ? right.score : right.results[sort.column]?.score
    if (leftScore == null && rightScore != null) return -factor
    if (leftScore != null && rightScore == null) return factor
    if (leftScore != null && rightScore != null && leftScore !== rightScore) {
      return (leftScore - rightScore) * factor
    }
    if (left.rank !== right.rank) return left.rank - right.rank
    return left.name.localeCompare(right.name, 'ja')
  })
}

export function mergeStandingProblemScores(
  problemScores: { problemId: string; score: number }[],
  details?: Record<string, StandingResultDetails>
): Record<string, ProblemResult> {
  return Object.fromEntries(
    problemScores.map(({ problemId, score }) => [
      problemId,
      {
        score: score === 0 && details && !(problemId in details) ? null : score,
        ...details?.[problemId]
      }
    ])
  )
}

export function formatAcceptedElapsedTime(
  acceptedAt: string | undefined,
  startsAt: Date
): string | null {
  if (!acceptedAt) return null
  const elapsedSeconds = Math.floor((Date.parse(acceptedAt) - startsAt.getTime()) / 1000)
  if (!Number.isFinite(elapsedSeconds) || elapsedSeconds < 0) return null
  const hours = Math.floor(elapsedSeconds / 3600)
  const minutes = Math.floor((elapsedSeconds % 3600) / 60)
  const seconds = String(elapsedSeconds % 60).padStart(2, '0')
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, '0')}:${seconds}`
    : `${minutes}:${seconds}`
}

export function getLastAcceptedAt(results: Record<string, ProblemResult>): string | undefined {
  let lastAcceptedAt: string | undefined
  let lastTime = Number.NEGATIVE_INFINITY
  for (const result of Object.values(results)) {
    if (!result.acceptedAt) continue
    const time = Date.parse(result.acceptedAt)
    if (!Number.isFinite(time) || time <= lastTime) continue
    lastAcceptedAt = result.acceptedAt
    lastTime = time
  }
  return lastAcceptedAt
}
