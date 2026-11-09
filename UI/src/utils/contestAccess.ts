import type { CurrentUser } from '@/api/client'

export const hasContestRole = (
  user: CurrentUser | null,
  contestId: string,
  role: 'contest-setter' | 'contestant'
) =>
  user?.roles.some(
    (assignment) =>
      assignment.role === role &&
      assignment.scope.kind === 'contest' &&
      assignment.scope.contestId === contestId
  ) ?? false
