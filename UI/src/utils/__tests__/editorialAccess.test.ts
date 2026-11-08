import { describe, expect, it } from 'vitest'
import type { GetMe200ResponseRolesInner } from '@/api/generated/contest/models/GetMe200ResponseRolesInner'
import {
  canViewEditorial,
  getEarlyEditorialAccess,
  parseEditorialRoles
} from '@/utils/editorialAccess'

function role(
  name: GetMe200ResponseRolesInner['role'],
  scope: GetMe200ResponseRolesInner['scope']
): GetMe200ResponseRolesInner {
  return { role: name, scope, userId: 'viewer', grantedAt: new Date('2026-10-01T00:00:00Z') }
}

describe('getEarlyEditorialAccess', () => {
  it('対象大会の運営者と担当解説だけを認識する', () => {
    const access = getEarlyEditorialAccess(
      [
        role('contest-admin', { kind: 'contest', contestId: 'other' }),
        role('contest-setter', { kind: 'contest', contestId: 'target' }),
        role('editiorial-author', { kind: 'editorial', editorialId: 'own' }),
        role('service-admin', { kind: 'service' })
      ],
      'target'
    )

    expect(access.isOrganizer).toBe(true)
    expect([...access.authoredEditorialIds]).toEqual(['own'])
  })
})

describe('parseEditorialRoles', () => {
  it('対象IDを含むscopeを保持する', () => {
    const roles = parseEditorialRoles({
      items: [
        { role: 'contest-setter', scope: { kind: 'contest', contestId: 'target' } },
        { role: 'editiorial-author', scope: { kind: 'editorial', editorialId: 'own' } }
      ]
    })
    const access = getEarlyEditorialAccess(roles, 'target')
    expect(access.isOrganizer).toBe(true)
    expect([...access.authoredEditorialIds]).toEqual(['own'])
  })
})

describe('canViewEditorial', () => {
  const publicEditorial = { id: 'public', authorId: 'other', visibility: 'public' } as const
  const privateEditorial = { id: 'private', authorId: 'writer', visibility: 'private' } as const

  it('終了前は一般利用者に公開解説も見せない', () => {
    const access = getEarlyEditorialAccess([], 'contest')
    expect(canViewEditorial(publicEditorial, false, access)).toBe(false)
    expect(canViewEditorial(privateEditorial, false, access)).toBe(false)
    expect(canViewEditorial(publicEditorial, true, access)).toBe(true)
    expect(canViewEditorial(privateEditorial, true, access)).toBe(false)
  })

  it('終了前でも担当解説または対象大会の運営者に見せる', () => {
    const author = getEarlyEditorialAccess(
      [role('editiorial-author', { kind: 'editorial', editorialId: 'private' })],
      'contest'
    )
    const organizer = getEarlyEditorialAccess(
      [role('contest-admin', { kind: 'contest', contestId: 'contest' })],
      'contest'
    )
    expect(canViewEditorial(privateEditorial, false, author)).toBe(true)
    expect(canViewEditorial(publicEditorial, false, author)).toBe(false)
    expect(canViewEditorial(privateEditorial, false, organizer)).toBe(true)
  })

  it('編集担当のロールを持たない著者にも担当解説を見せる', () => {
    const author = getEarlyEditorialAccess([], 'contest', 'writer')
    expect(canViewEditorial(privateEditorial, false, author)).toBe(true)
    expect(canViewEditorial(publicEditorial, false, author)).toBe(false)
  })
})
