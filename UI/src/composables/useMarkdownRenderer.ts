import markdownItKatex from '@vscode/markdown-it-katex'
import DOMPurify from 'dompurify'
import MarkdownIt from 'markdown-it'
import { type BundledLanguage, createHighlighter, type Highlighter } from 'shiki'
import { computed, type ComputedRef, shallowRef } from 'vue'

// CommonJSのdefaultは、Vitestでは関数、Viteではdefaultプロパティを持つオブジェクトになる。
const katexPlugin = markdownItKatex as typeof markdownItKatex | { default: typeof markdownItKatex }

// コードエディターと見た目を揃える。
const THEME = 'github-light'

// 初期化を軽くするため、よく使う言語だけを先に読み込む。
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

export function useMarkdownRenderer(): {
  renderMarkdown: (source: string, typographer?: boolean) => string
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
    html: false,
    linkify: true,
    typographer: false,
    highlight
  })
  md.use(typeof katexPlugin === 'function' ? katexPlugin : katexPlugin.default, {
    throwOnError: false,
    trust: false
  })

  const renderMarkdown = (source: string, typographer = false): string => {
    const rawHtml = md.set({ typographer }).render(source)
    // KaTeX の SVG・MathML・インラインスタイルを保ち，ページ全体に及ぶ CSS は除去する．
    return DOMPurify.sanitize(rawHtml, { FORBID_TAGS: ['style'] })
  }

  initHighlighter()

  return { renderMarkdown, isReady }
}
