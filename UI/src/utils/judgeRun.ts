import type { JudgeRun } from '@/api/client'

export function isJudgeRunFinished(run: JudgeRun | null | undefined): boolean {
  return run?.state === 'completed' || run?.state === 'failed'
}

export function latestJudgeRun(current: JudgeRun | null | undefined, incoming: JudgeRun): JudgeRun {
  return current?.id === incoming.id && current.revision >= incoming.revision ? current : incoming
}

export const judgeRunStateLabels: Record<JudgeRun['state'], string> = {
  queued: '採点待ち',
  preparing: '採点準備中',
  running: '採点中',
  aggregating: '結果集計中',
  completed: '採点完了',
  failed: '採点失敗'
}
