export type StandingsKind = 'individual' | 'team' | 'extended'

export type ProblemResult = {
  score: number | null
  pendingCount?: number
  acceptedAt?: string
}

export type StandingResultDetails = Omit<ProblemResult, 'score'>

export type FirstAccept = {
  id: string
  name: string
  acceptedAt?: string
}

export type ProblemStat = {
  solvedCount: number
  submittedCount: number
}

export type StandingRow = {
  id: string
  name: string
  rank: number
  score: number
  members?: string[]
  memberIds?: string[]
  results: Record<string, ProblemResult>
}

export type StandingBoard = {
  items: StandingRow[]
  updatedAt: Date
  isFrozen: boolean
  frozenAt: Date | null
  problemStats?: Record<string, ProblemStat>
  firstAccepts?: Record<string, FirstAccept>
}

// 順位APIにまだない画面情報を補う仮の応答．得点と順位は正式なAPIから取得する．
export type IndividualStandingDetails = {
  isFrozen: boolean
  frozenAt: string | null
  problemStats?: Record<string, ProblemStat>
  firstAccepts?: Record<string, FirstAccept>
  users: {
    userId: string
    name: string
    results: Record<string, StandingResultDetails>
  }[]
}

export type TeamStandingDetails = {
  isFrozen: boolean
  frozenAt: string | null
  problemStats?: Record<string, ProblemStat>
  firstAccepts?: Record<string, FirstAccept>
  teams: {
    teamId: string
    results: Record<string, StandingResultDetails>
  }[]
}

// 延長戦の仮の応答．正式なAPI契約が決まったら置き換える．
export type PreviewStandingBoard = {
  items: StandingRow[]
  updatedAt: string
  isFrozen: boolean
  frozenAt: string | null
  problemStats?: Record<string, ProblemStat>
  firstAccepts?: Record<string, FirstAccept>
}
