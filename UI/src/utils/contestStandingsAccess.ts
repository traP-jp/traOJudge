import type { MeApi } from '@/api/generated/contest/apis/MeApi'
import type { GetContest200Response as Contest } from '@/api/generated/contest/models/GetContest200Response'
import type { StandingsKind } from '@/types/standings'

export type ContestViewerRole = {
  role: string
  scope: { kind: string; contestId?: string }
}

export async function loadContestViewerRoles(
  api: MeApi,
  signal: AbortSignal
): Promise<ContestViewerRole[]> {
  const response = await api.listMyRolesRaw({ signal })
  const payload: unknown = await response.raw.json()
  if (payload === null || typeof payload !== 'object' || !('items' in payload)) {
    throw new Error('Invalid roles response')
  }
  if (!Array.isArray(payload.items)) throw new Error('Invalid roles response')
  return payload.items.filter((item: unknown): item is ContestViewerRole => {
    if (item === null || typeof item !== 'object') return false
    if (!('role' in item) || typeof item.role !== 'string') return false
    if (!('scope' in item) || item.scope === null || typeof item.scope !== 'object') return false
    return 'kind' in item.scope && typeof item.scope.kind === 'string'
  })
}

export function canViewContestStandings(
  contest: Contest,
  kind: StandingsKind,
  now: number,
  roles: ContestViewerRole[]
): boolean {
  if (kind === 'extended' && now < contest.endsAt.getTime()) return false
  if (contest.visibility === 'public') return true
  return roles.some(
    ({ role, scope }) =>
      scope.kind === 'contest' &&
      scope.contestId === contest.id &&
      (role === 'contestant' || role === 'contest-admin' || role === 'contest-setter')
  )
}
