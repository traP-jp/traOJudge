import { computed, shallowRef, type ComputedRef } from 'vue'
import MarkdownIt from 'markdown-it'
import markdownItKatex from '@vscode/markdown-it-katex'
import DOMPurify from 'dompurify'
import { createHighlighter, type BundledLanguage, type Highlighter } from 'shiki'

// CodeBlock (提出フォーム) と見た目を揃えるため同じテーマを使う
const THEME = 'github-light'

// 問題文で頻出する言語のみ読み込み，未対応の言語はプレーンテキストで表示する．
const PRELOADED_LANGS: BundledLanguage[] = [
  'cpp',
  'c',
  'java',
  'python',
  'javascript',
  'typescript',
  'ruby',
  'swift',
  'go',
  'rust',
  'kotlin',
  'bash',
  'json',
  'html',
  'css',
  'sql'
]

// 画面の切り替えやプレビューの再表示で WASM と言語定義を繰り返し初期化しない．
const highlighter = shallowRef<Highlighter>()
const isReady = computed(() => highlighter.value !== undefined)
let initialization: Promise<void> | undefined

function initHighlighter(): void {
  if (initialization) return

  initialization = createHighlighter({
    themes: [THEME],
    langs: PRELOADED_LANGS
  })
    .then((value) => {
      highlighter.value = value
    })
    .catch((error: unknown) => {
      // 通信失敗時も本文は表示し，次に画面を開いたときに再試行する．
      console.warn('コードハイライトの初期化に失敗しました:', error)
      initialization = undefined
    })
}

/**
 * Markdown に数式とコードの色分けを適用し，危険な HTML を除去する．
 * computed 内で呼び出すと，ハイライターの初期化後にも表示を更新する．
 */
export function useMarkdownRenderer(): {
  renderMarkdown: (source: string) => string
  isReady: ComputedRef<boolean>
} {
  // markdown-it の同期 API に合わせ，初期化が終わるまでは通常のコード表示を使う．
  function highlight(code: string, lang: string): string {
    const current = highlighter.value
    if (current && lang && current.getLoadedLanguages().includes(lang)) {
      return current.codeToHtml(code, { lang, theme: THEME })
    }
    // 空文字を返すと markdown-it がコードをエスケープして表示する．
    return ''
  }

  const md = new MarkdownIt({
    // HTML を許可するため，数式やハイライトの変換後に必ずサニタイズする．
    html: true,
    linkify: true,
    typographer: true,
    highlight
  })
  md.use(markdownItKatex, { throwOnError: false, trust: false })

  const renderMarkdown = (source: string): string => {
    const rawHtml = md.render(source)
    // KaTeX の SVG・MathML・インラインスタイルを保ち，ページ全体に及ぶ CSS は除去する．
    return DOMPurify.sanitize(rawHtml, { FORBID_TAGS: ['style'] })
  }

  initHighlighter()

  return { renderMarkdown, isReady }
}
