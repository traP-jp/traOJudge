import { createApp, h, ref } from 'vue'
import type { languages } from 'monaco-editor'
import { describe, expect, it, vi } from 'vitest'
import CodeBlock from '@/components/CodeBlock.vue'

const editor = vi.hoisted(() => ({
  providers: new Map<string, languages.TokensProvider>(),
  languageIds: new Set<string>(),
  setModelLanguage: vi.fn(),
  dispose: vi.fn()
}))

// Keep Shiki and its Monaco adapter real; only replace the browser editor surface.
vi.mock('monaco-editor', () => ({
  editor: {
    create: () => ({
      getModel: () => ({ onDidChangeContent: vi.fn() }),
      dispose: editor.dispose
    }),
    defineTheme: vi.fn(),
    setTheme: vi.fn(),
    setModelLanguage: editor.setModelLanguage
  },
  languages: {
    register: ({ id }: { id: string }) => editor.languageIds.add(id),
    getLanguages: () => [...editor.languageIds].map((id) => ({ id })),
    setTokensProvider: (id: string, provider: languages.TokensProvider) =>
      editor.providers.set(id, provider),
    typescript: { typescriptDefaults: { setEagerModelSync: vi.fn() } }
  }
}))

describe('CodeBlock syntax highlighting', () => {
  it('highlights the selected language and loads another grammar after switching', async () => {
    const language = ref({ id: 'cpp', name: 'C++ 20' })
    const app = createApp({ setup: () => () => h(CodeBlock, { language: language.value }) })
    const container = document.createElement('div')
    const previousEnvironment = self.MonacoEnvironment
    app.mount(container)

    try {
      await vi.waitFor(
        () => expect(editor.setModelLanguage).toHaveBeenLastCalledWith(expect.anything(), 'cpp'),
        { timeout: 10000 }
      )

      const cpp = editor.providers.get('cpp')!
      const cppTokens = cpp.tokenize('int main() { return 0; }', cpp.getInitialState()).tokens
      expect(cppTokens.some((token) => token.scopes.length > 0)).toBe(true)

      language.value = { id: 'python', name: 'Python 3' }
      await vi.waitFor(() =>
        expect(editor.setModelLanguage).toHaveBeenLastCalledWith(expect.anything(), 'python')
      )

      const python = editor.providers.get('python')!
      const pythonTokens = python.tokenize(
        'def solve(): return 42',
        python.getInitialState()
      ).tokens
      expect(pythonTokens.some((token) => token.scopes.length > 0)).toBe(true)

      language.value = { id: 'unknown', name: 'Unknown language' }
      await vi.waitFor(() =>
        expect(editor.setModelLanguage).toHaveBeenLastCalledWith(expect.anything(), 'text')
      )
    } finally {
      app.unmount()
      self.MonacoEnvironment = previousEnvironment
    }

    expect(editor.dispose).toHaveBeenCalledOnce()
  })
})
