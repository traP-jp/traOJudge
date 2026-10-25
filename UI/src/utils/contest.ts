export type ContestPhase = 'upcoming' | 'running' | 'finished'

export function getContestPhase(
  startsAt: Date | string,
  endsAt: Date | string,
  now: number
): ContestPhase {
  if (now < new Date(startsAt).getTime()) return 'upcoming'
  if (now < new Date(endsAt).getTime()) return 'running'
  return 'finished'
}

export function formatContestDuration(milliseconds: number): string {
  const minutes = Math.max(0, Math.ceil(milliseconds / 60000))
  const days = Math.floor(minutes / 1440)
  const hours = Math.floor((minutes % 1440) / 60)
  const remainingMinutes = minutes % 60
  const parts = []
  if (days > 0) parts.push(`${days}日`)
  if (hours > 0) parts.push(`${hours}時間`)
  if (remainingMinutes > 0 || parts.length === 0) parts.push(`${remainingMinutes}分`)
  return parts.join(' ')
}

export function formatContestDate(date: Date | string): string {
  return new Intl.DateTimeFormat('ja-JP', {
    timeZone: 'Asia/Tokyo',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    weekday: 'short',
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23'
  }).format(new Date(date))
}
