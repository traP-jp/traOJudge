import { effectScope, nextTick, ref } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import { useCursorPagination } from '@/composables/useCursorPagination'

const flush = async () => {
  await nextTick()
  await Promise.resolve()
  await nextTick()
}
describe('useCursorPagination', () => {
  it('uses opaque cursors for next/previous pages and resets on search changes', async () => {
    const key = ref('first')
    const load = vi.fn(async (cursor: string | undefined) => ({
      items: [cursor ?? key.value],
      nextCursor: cursor ? null : 'opaque-cursor'
    }))
    const scope = effectScope()
    const pages = scope.run(() => useCursorPagination(load, key))!
    await flush()
    expect(pages.items.value).toEqual(['first'])
    pages.next()
    await flush()
    expect(load.mock.lastCall?.[0]).toBe('opaque-cursor')
    expect(pages.page.value).toBe(2)
    expect(pages.hasNext.value).toBe(false)
    pages.previous()
    await flush()
    expect(load.mock.lastCall?.[0]).toBeUndefined()
    key.value = 'second'
    await flush()
    expect(pages.page.value).toBe(1)
    expect(pages.items.value).toEqual(['second'])
    scope.stop()
  })
  it('discards a response from an obsolete search', async () => {
    const key = ref('first')
    const requests: {
      signal: AbortSignal
      resolve: (value: { items: string[]; nextCursor: null }) => void
    }[] = []
    const scope = effectScope()
    const pages = scope.run(() =>
      useCursorPagination<string>(
        (_, signal) => new Promise((resolve) => requests.push({ signal, resolve })),
        key
      )
    )!
    key.value = 'second'
    await flush()
    expect(requests[0].signal.aborted).toBe(true)
    requests[1].resolve({ items: ['new'], nextCursor: null })
    await flush()
    requests[0].resolve({ items: ['stale'], nextCursor: null })
    await flush()
    expect(pages.items.value).toEqual(['new'])
    scope.stop()
  })
  it('keeps the current page when the next request fails and permits retry', async () => {
    const load = vi
      .fn()
      .mockResolvedValueOnce({ items: ['first'], nextCursor: 'second' })
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce({ items: ['second'], nextCursor: null })
    const scope = effectScope()
    const pages = scope.run(() => useCursorPagination<string>(load, ref('key')))!
    await flush()
    pages.next()
    await flush()
    expect(pages.page.value).toBe(1)
    expect(pages.error.value).toBe('offline')
    pages.next()
    await flush()
    expect(pages.items.value).toEqual(['second'])
    scope.stop()
  })
})
