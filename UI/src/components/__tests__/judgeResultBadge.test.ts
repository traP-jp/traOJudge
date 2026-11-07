import { afterEach, describe, expect, it } from 'vitest'
import { createApp, type App } from 'vue'
import JudgeResultBadge from '@/components/JudgeResultBadge.vue'
import type { JudgeStatus } from '@/utils/statusConfig'

let app: App | undefined
let host: HTMLDivElement | undefined
afterEach(() => {
  app?.unmount()
  host?.remove()
})

function mount(status: JudgeStatus, progress?: { completed: number; total: number | null }) {
  host = document.createElement('div')
  document.body.append(host)
  app = createApp(JudgeResultBadge, { status, progress })
  app.mount(host)
  return host.querySelector<HTMLElement>('[role="status"]')!
}

describe('JudgeResultBadge', () => {
  it.each(['WJ', 'AC'] as const)('keeps %s grey until judging is complete', (status) => {
    const badge = mount(status, { completed: 1, total: 3 })
    expect(badge.textContent).toBe('WJ 1/3...')
    expect(badge.classList.contains('bg-status-pending')).toBe(true)
    const dots = badge.querySelectorAll('[aria-hidden="true"] > span')
    expect(dots).toHaveLength(3)
    expect([...dots].map((dot) => (dot as HTMLElement).style.animationDelay)).toEqual([
      '0s',
      '0.2s',
      '0.4s'
    ])
    expect([...dots].every((dot) => dot.classList.contains('motion-safe:animate-judge-dot'))).toBe(
      true
    )
  })

  it.each(['WA', 'CE', 'TLE', 'MLE', 'RE', 'IE', 'WE'] as const)(
    'shows an intermediate %s with its colour and keeps the progress indicator',
    (status) => {
      const badge = mount(status, { completed: 2, total: 3 })
      expect(badge.textContent).toBe(`${status} 2/3...`)
      expect(badge.classList.contains('bg-status-pending')).toBe(false)
      expect(badge.querySelector('[aria-hidden="true"]')).not.toBeNull()
    }
  )

  it('uses an unknown denominator without inventing a total', () => {
    const badge = mount('WJ', { completed: 1, total: null })
    expect(badge.textContent).toBe('WJ 1/?...')
    expect(badge.getAttribute('aria-label')).toContain('総数不明')
  })

  it.each(['AC', 'WA', 'CE'] as const)('shows only %s after completion', (status) => {
    const badge = mount(status)
    expect(badge.textContent).toBe(status)
    expect(badge.querySelector('[aria-hidden="true"]')).toBeNull()
    expect(badge.classList.contains('bg-status-pending')).toBe(false)
  })
})
