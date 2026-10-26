import type { MeApi } from '@/api/generated/contest/apis/MeApi'
import type { GetMe200ResponseRolesInner } from '@/api/generated/contest/models/GetMe200ResponseRolesInner'

type EditorialRole = Pick<GetMe200ResponseRolesInner, 'role' | 'scope'>

export type EarlyEditorialAccess = {
  isOrganizer: boolean
  authoredEditorialIds: Set<string>
  viewerId: string
}

export async function loadMyEditorialRoles(
  api: MeApi,
  signal: AbortSignal
): Promise<EditorialRole[]> {
  // 生成クライアントのoneOf変換はscopeのIDを落とすため，応答の元データを読む．
  const response = await api.listMyRolesRaw({ signal })
  const payload: unknown = await response.raw.json()
  return parseEditorialRoles(payload)
}

export function parseEditorialRoles(payload: unknown): EditorialRole[] {
  if (payload === null || typeof payload !== 'object' || !('items' in payload)) {
    throw new Error('Invalid roles response')
  }
  if (!Array.isArray(payload.items)) throw new Error('Invalid roles response')
  const valid = payload.items.every(
    (item: unknown) =>
      item !== null &&
      typeof item === 'object' &&
      'role' in item &&
      typeof item.role === 'string' &&
      'scope' in item &&
      item.scope !== null &&
      typeof item.scope === 'object' &&
      'kind' in item.scope &&
      typeof item.scope.kind === 'string'
  )
  if (!valid) throw new Error('Invalid roles response')
  return payload.items as EditorialRole[]
}

export function getEarlyEditorialAccess(
  roles: EditorialRole[],
  contestId: string,
  viewerId = ''
): EarlyEditorialAccess {
  const isOrganizer = roles.some(
    (role) =>
      (role.role === 'contest-admin' || role.role === 'contest-setter') &&
      role.scope.kind === 'contest' &&
      'contestId' in role.scope &&
      role.scope.contestId === contestId
  )
  const authoredEditorialIds = new Set(
    roles
      .filter(
        (role) =>
          role.role === 'editiorial-author' &&
          role.scope.kind === 'editorial' &&
          'editorialId' in role.scope
      )
      .map((role) => ('editorialId' in role.scope ? role.scope.editorialId : ''))
  )
  return { isOrganizer, authoredEditorialIds, viewerId }
}

export function canViewEditorial(
  editorial: { id: string; authorId: string; visibility: 'public' | 'private' },
  hasFinished: boolean,
  access: EarlyEditorialAccess
): boolean {
  return (
    access.isOrganizer ||
    (access.viewerId !== '' && editorial.authorId === access.viewerId) ||
    access.authoredEditorialIds.has(editorial.id) ||
    (hasFinished && editorial.visibility === 'public')
  )
}
