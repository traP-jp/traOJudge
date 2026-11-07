import type {
  IndividualStandingDetails,
  PreviewStandingBoard,
  StandingBoard,
  TeamStandingDetails
} from '@/types/standings'

export class StandingsPreviewError extends Error {
  constructor(public readonly status: number) {
    super(`Standings preview request failed: ${status}`)
  }
}

const basePath = (import.meta.env.VITE_CONTEST_API_BASE_URL || import.meta.env.VITE_API_BASE_URL || '/api/v1').replace(/\/$/, '')

async function getPreview<T>(path: string, signal: AbortSignal): Promise<T> {
  const response = await fetch(`${basePath}${path}`, { credentials: 'include', signal })
  if (!response.ok) throw new StandingsPreviewError(response.status)
  return (await response.json()) as T
}

export function getIndividualStandingDetails(
  contestId: string,
  signal: AbortSignal
): Promise<IndividualStandingDetails> {
  return getPreview(`/contests/${encodeURIComponent(contestId)}/standings/details`, signal)
}

export function getTeamStandingDetails(
  contestId: string,
  signal: AbortSignal
): Promise<TeamStandingDetails> {
  return getPreview(`/contests/${encodeURIComponent(contestId)}/standings/team/details`, signal)
}

export async function getExtendedStandingBoard(
  contestId: string,
  signal: AbortSignal
): Promise<StandingBoard> {
  const response = await getPreview<PreviewStandingBoard>(
    `/contests/${encodeURIComponent(contestId)}/standings/extended`,
    signal
  )
  return {
    items: response.items,
    updatedAt: new Date(response.updatedAt),
    isFrozen: response.isFrozen,
    frozenAt: response.frozenAt ? new Date(response.frozenAt) : null,
    problemStats: response.problemStats,
    firstAccepts: response.firstAccepts
  }
}
