import * as monaco from 'monaco-editor'
import { expect, it, vi } from 'vitest'

import { getSyntaxName, initializeCodeHighlighter } from '@/components/codeHighlighter'

vi.mock('monaco-editor', () => ({
  editor: { defineTheme: vi.fn(), setTheme: vi.fn() },
  languages: {
    register: vi.fn(),
    getLanguages: () => [{ id: 'javascript' }],
    setTokensProvider: vi.fn()
  }
}))

it('keeps real Shiki themes and tokenizers usable across repeated editor initialization', async () => {
  await Promise.all([initializeCodeHighlighter(), initializeCodeHighlighter()])
  const setTheme = monaco.editor.setTheme
  await initializeCodeHighlighter()
  expect(monaco.editor.setTheme).toBe(setTheme)
  expect(() => monaco.editor.setTheme('github-light')).not.toThrow()
  const provider = vi.mocked(monaco.languages.setTokensProvider).mock.calls[0]?.[1]
  if (!provider || !('tokenize' in provider) || !provider.tokenize)
    throw new Error('Tokenizer was not registered')
  expect(
    provider.tokenize('const answer = 42', provider.getInitialState()).tokens.length
  ).toBeGreaterThan(1)
  expect(monaco.editor.defineTheme).toHaveBeenCalledTimes(1)
})

it('matches JavaScript separately from Java and keeps unknown languages plain', () => {
  expect(getSyntaxName('JavaScript (Node.js)')).toBe('javascript')
  expect(getSyntaxName('Java 21')).toBe('java')
  expect(getSyntaxName('C++ 23')).toBe('cpp')
  expect(getSyntaxName('unknown')).toBeUndefined()
})
