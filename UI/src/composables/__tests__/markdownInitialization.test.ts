import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createHighlighter } from 'shiki'

vi.mock('shiki', () => ({ createHighlighter: vi.fn() }))

beforeEach(() => {
  vi.resetModules()
  vi.mocked(createHighlighter).mockReset()
})

describe('Markdown highlighter initialization', () => {
  it('shares pending initialization across renderer instances', async () => {
    vi.mocked(createHighlighter).mockReturnValue(new Promise(() => {}))
    const { useMarkdownRenderer } = await import('@/composables/useMarkdownRenderer')
    const first = useMarkdownRenderer()
    const second = useMarkdownRenderer()

    expect(createHighlighter).toHaveBeenCalledTimes(1)
    expect(first.isReady.value).toBe(false)
    expect(second.renderMarkdown('```cpp\nint x;\n```')).toContain(
      '<code class="language-cpp">int x;'
    )
  })

  it('keeps rendering after initialization fails and retries on the next mount', async () => {
    vi.mocked(createHighlighter).mockRejectedValueOnce(new Error('Network unavailable'))
    const warning = vi.spyOn(console, 'warn').mockImplementation(() => {})
    try {
      const { useMarkdownRenderer } = await import('@/composables/useMarkdownRenderer')
      const first = useMarkdownRenderer()
      await vi.waitFor(() => expect(warning).toHaveBeenCalledOnce())
      expect(first.isReady.value).toBe(false)
      expect(first.renderMarkdown('```cpp\nint x;\n```')).toContain('int x;')
      expect(first.renderMarkdown('$x$')).toContain('katex')

      vi.mocked(createHighlighter).mockReturnValue(new Promise(() => {}))
      useMarkdownRenderer()
      expect(createHighlighter).toHaveBeenCalledTimes(2)
    } finally {
      warning.mockRestore()
    }
  })
})
