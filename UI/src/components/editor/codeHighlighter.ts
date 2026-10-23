import { shikiToMonaco } from '@shikijs/monaco'
import * as monaco from 'monaco-editor'
import { type BundledLanguage, createHighlighter } from 'shiki'

const syntaxes: [string, BundledLanguage][] = [
  ['C++', 'cpp'],
  ['C', 'c'],
  ['JavaScript', 'javascript'],
  ['Java', 'java'],
  ['Python', 'python'],
  ['Ruby', 'ruby'],
  ['Swift', 'swift'],
  ['Go', 'go'],
  ['Rust', 'rust'],
  ['Kotlin', 'kotlin']
]

export const getSyntaxName = (name: string): BundledLanguage | undefined =>
  syntaxes.find(([prefix]) => name.startsWith(prefix))?.[1]

// Monacoのテーマとトークナイザーはグローバルなので、highlighterも同じ寿命で共有する。
let initialization: Promise<void> | undefined
export const initializeCodeHighlighter = (): Promise<void> => {
  initialization ??= createHighlighter({
    themes: ['github-light'],
    langs: syntaxes.map(([, syntax]) => syntax)
  })
    .then((highlighter) => {
      for (const [, id] of syntaxes) monaco.languages.register({ id })
      shikiToMonaco(highlighter, monaco)
    })
    .catch((error: unknown) => {
      initialization = undefined
      throw error
    })
  return initialization
}
