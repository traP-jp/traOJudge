import { describe, expect, it, vi } from 'vitest'
import { computed, createApp, h, nextTick, ref } from 'vue'

import MarkdownContent from '@/components/MarkdownContent.vue'
import { useMarkdownRenderer } from '@/composables/useMarkdownRenderer'

const fragment = (html: string): DocumentFragment => {
  const template = document.createElement('template')
  template.innerHTML = html
  return template.content
}

describe('Markdown rendering', () => {
  it('updates computed output when highlighting becomes available', async () => {
    const { renderMarkdown, isReady } = useMarkdownRenderer()
    const output = computed(() => renderMarkdown('```cpp\nint main() { return 0; }\n```'))

    expect(fragment(output.value).querySelector('pre code')?.textContent).toContain('int main()')
    await vi.waitFor(() => expect(isReady.value).toBe(true), { timeout: 10000 })
    expect(fragment(output.value).querySelector('pre.shiki code span[style]')).not.toBeNull()
  })

  it('renders headings, lists, links, and tables', () => {
    const { renderMarkdown } = useMarkdownRenderer()
    const output = fragment(
      renderMarkdown(
        '# Title\n\n- Item\n\n[Link](https://example.com)\n\n| A | B |\n| - | - |\n| 1 | 2 |'
      )
    )
    expect(output.querySelector('h1')?.textContent).toBe('Title')
    expect(output.querySelector('li')?.textContent).toBe('Item')
    expect(output.querySelector('a')?.getAttribute('href')).toBe('https://example.com')
    expect(output.querySelectorAll('td')).toHaveLength(2)
  })

  it('preserves inline and display math, MathML, and KaTeX SVG', () => {
    const { renderMarkdown } = useMarkdownRenderer()
    const output = fragment(renderMarkdown('$a^2 + b^2$\n\n$$\n\\sqrt{x} + \\widehat{abc}\n$$'))

    expect(output.querySelectorAll('.katex')).toHaveLength(2)
    expect(output.querySelector('.katex-display math')).not.toBeNull()
    expect(output.querySelector('.katex-html svg path')).not.toBeNull()
    expect(output.querySelector('.katex-html [style]')).not.toBeNull()
  })

  it('places article headings below the page title without changing default rendering or code', () => {
    const { renderMarkdown } = useMarkdownRenderer()
    const source = '## 入力\n\n### 形式\n\n###### 詳細\n\n```text\n# 入力例\n```'
    const output = fragment(renderMarkdown(source, false, 3))
    expect(
      [...output.querySelectorAll('h3, h4, h6')].map((heading) => heading.textContent)
    ).toEqual(['入力', '形式', '詳細'])
    expect(output.querySelector('h1, h2, h7')).toBeNull()
    expect(output.querySelector('code')?.textContent).toBe('# 入力例\n')
    expect(fragment(renderMarkdown('# 題名', false, 3)).querySelector('h3')?.textContent).toBe(
      '題名'
    )
    expect(fragment(renderMarkdown(source)).querySelector('h2')?.textContent).toBe('入力')
    expect(fragment(renderMarkdown('本文だけ', false, 3)).querySelector('p')?.textContent).toBe(
      '本文だけ'
    )
  })

  it('keeps malformed math and unclosed delimiters readable', () => {
    const { renderMarkdown } = useMarkdownRenderer()
    expect(fragment(renderMarkdown('$\\frac{1}{$')).querySelector('.katex-error')).not.toBeNull()
    expect(fragment(renderMarkdown('Unfinished $x')).textContent).toContain('Unfinished $x')
    expect(renderMarkdown('')).toBe('')
  })

  it('escapes code with unknown or missing languages and leaves math in code untouched', () => {
    const { renderMarkdown } = useMarkdownRenderer()
    for (const language of ['', 'unknown-language']) {
      const output = fragment(
        renderMarkdown('```' + language + '\n<script>alert(1)</script> $x$\n```')
      )
      expect(output.querySelector('code')?.textContent).toBe('<script>alert(1)</script> $x$\n')
      expect(output.querySelector('script, .katex')).toBeNull()
    }
  })

  it('removes scripts, event handlers, unsafe URLs, frames, and global styles', () => {
    const { renderMarkdown } = useMarkdownRenderer()
    const output = fragment(
      renderMarkdown(
        '<p>Safe</p><script>alert(1)</script><style>body { display: none }</style>' +
          '<img src="x" onerror="alert(1)"><a href="javascript:alert(1)">link</a>' +
          '<iframe srcdoc="<script>alert(1)</script>"></iframe>' +
          '<svg onload="alert(1)"><a href="javascript:alert(1)">svg</a></svg>'
      )
    )
    expect(output.querySelector('p')?.textContent).toContain('<p>Safe</p>')
    expect(output.querySelector('script, style, iframe, [onerror], [onload]')).toBeNull()
    expect(
      [...output.querySelectorAll('[href]')].some((node) =>
        node.getAttribute('href')?.startsWith('javascript:')
      )
    ).toBe(false)
  })

  it('does not enable trusted HTML commands in math', () => {
    const { renderMarkdown } = useMarkdownRenderer()
    const output = fragment(renderMarkdown('$\\href{javascript:alert(1)}{click}$'))
    expect(output.querySelector('a')).toBeNull()
  })

  it('updates the shared display component when the source changes', async () => {
    const source = ref('# Original')
    const container = document.createElement('div')
    const app = createApp({ setup: () => () => h(MarkdownContent, { source: source.value }) })
    app.mount(container)
    try {
      expect(container.querySelector('h1')?.textContent).toBe('Original')
      source.value = '$x^2$'
      await nextTick()
      expect(container.querySelector('h1')).toBeNull()
      expect(container.querySelector('.katex')).not.toBeNull()
      source.value = ''
      await nextTick()
      expect(container.textContent).toBe('')
    } finally {
      app.unmount()
    }
  })
})
