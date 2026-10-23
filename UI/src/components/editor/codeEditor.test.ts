import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { deferred } from '@/test/helpers'

const mock = vi.hoisted(() => {
  let onChange = () => {}
  const model = {
    dispose: vi.fn(),
    onDidChangeContent: vi.fn((callback: () => void) => {
      onChange = callback
    })
  }
  const editor = {
    getModel: () => model,
    getValue: () => 'typed before highlighting',
    dispose: vi.fn(),
    setValue: vi.fn()
  }
  return { model, editor, change: () => onChange(), highlighter: vi.fn(), setLanguage: vi.fn() }
})
vi.mock('monaco-editor', () => ({
  editor: { create: () => mock.editor, setModelLanguage: mock.setLanguage },
  languages: {
    register: vi.fn(),
    typescript: { typescriptDefaults: { setEagerModelSync: vi.fn() } }
  }
}))
vi.mock('shiki', () => ({ createHighlighter: mock.highlighter }))
vi.mock('@shikijs/monaco', () => ({ shikiToMonaco: vi.fn() }))
vi.mock('monaco-editor/esm/vs/editor/editor.worker?worker', () => ({ default: class {} }))
vi.mock('monaco-editor/esm/vs/language/json/json.worker?worker', () => ({ default: class {} }))
vi.mock('monaco-editor/esm/vs/language/css/css.worker?worker', () => ({ default: class {} }))
vi.mock('monaco-editor/esm/vs/language/html/html.worker?worker', () => ({ default: class {} }))
vi.mock('monaco-editor/esm/vs/language/typescript/ts.worker?worker', () => ({ default: class {} }))
beforeEach(() => vi.resetModules())
afterEach(() => vi.clearAllMocks())

it('captures input while syntax highlighting is still initializing', async () => {
  const highlighter = { loadLanguage: vi.fn().mockResolvedValue(undefined), dispose: vi.fn() }
  const pending = deferred<typeof highlighter>()
  mock.highlighter.mockReturnValue(pending.promise)
  const { default: CodeEditor } = await import('@/components/editor/CodeEditor.vue')
  const wrapper = mount(CodeEditor, { props: { language: { id: 'cpp', name: 'C++' } } })
  mock.change()
  expect(wrapper.emitted('update:modelValue')).toEqual([['typed before highlighting']])
  pending.resolve(highlighter)
  await flushPromises()
  wrapper.unmount()
  expect(mock.model.dispose).toHaveBeenCalledTimes(1)
  expect(highlighter.dispose).not.toHaveBeenCalled()
})

it('does not touch an unmounted editor when shared highlighting becomes ready', async () => {
  const highlighter = { loadLanguage: vi.fn().mockResolvedValue(undefined), dispose: vi.fn() }
  const pending = deferred<typeof highlighter>()
  mock.highlighter.mockReturnValue(pending.promise)
  const { default: CodeEditor } = await import('@/components/editor/CodeEditor.vue')
  const wrapper = mount(CodeEditor, { props: { language: { id: 'cpp', name: 'C++' } } })
  wrapper.unmount()
  pending.resolve(highlighter)
  await flushPromises()
  expect(highlighter.dispose).not.toHaveBeenCalled()
  expect(mock.setLanguage).not.toHaveBeenCalled()
})
