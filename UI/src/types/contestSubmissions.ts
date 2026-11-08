import type { ContestProblem } from '@/api/generated'

export type ContestSubmissionScope = 'all' | 'mine' | 'team'

export type ContestSubmissionProblem = ContestProblem & {
  title: string
  canOpen: boolean
}

export type ContestSubmissionTeam = {
  id: string
  name: string
  members: { userId: string; displayName: string }[]
}
