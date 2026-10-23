<script setup lang="ts">
import * as monaco from 'monaco-editor'
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker'
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker'
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker'
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker'
import { onBeforeUnmount, onMounted, useTemplateRef, watch } from 'vue'

import { type Language } from '@/api/generated'
import { getSyntaxName, initializeCodeHighlighter } from '@/components/codeHighlighter'

const { language } = defineProps<{
  language: Language
}>()

const model = defineModel<string | null>({ required: false })

let editor: monaco.editor.IStandaloneCodeEditor | undefined
const element = useTemplateRef<HTMLElement>('codeBlock')

let disposed = false
let ready = false

const setLanguage = () => {
  if (!ready || disposed) return
  const currentModel = editor?.getModel()
  if (currentModel)
    monaco.editor.setModelLanguage(currentModel, getSyntaxName(language.name) ?? 'plaintext')
}

const configureMonacoEditor = () => {
  self.MonacoEnvironment = {
    getWorker(_, label) {
      if (label === 'json') {
        return new jsonWorker()
      }
      if (label === 'css' || label === 'scss' || label === 'less') {
        return new cssWorker()
      }
      if (label === 'html' || label === 'handlebars' || label === 'razor') {
        return new htmlWorker()
      }
      if (label === 'typescript' || label === 'javascript') {
        return new tsWorker()
      }
      return new editorWorker()
    }
  }
  monaco.languages.typescript.typescriptDefaults.setEagerModelSync(true)
}

onMounted(async () => {
  if (import.meta.env.SSR) return

  configureMonacoEditor()

  editor = monaco.editor.create(element.value!, {
    value: model.value ?? '',
    automaticLayout: true,
    theme: 'github-light'
  })

  // ハイライトの準備が終わる前の入力もフォームへ反映する。
  editor.getModel()?.onDidChangeContent(() => {
    model.value = editor!.getValue()
  })

  await initializeCodeHighlighter()
  ready = true
  setLanguage()
})

watch(() => language, setLanguage)

onBeforeUnmount(() => {
  disposed = true
  const editorModel = editor?.getModel()
  editor?.dispose()
  editorModel?.dispose()
})

watch(model, () => {
  const value = model.value ?? ''
  if (value != editor?.getValue()) editor?.setValue(value)
})
</script>

<template>
  <div class="rounded-sm border border-border-primary p-1">
    <div ref="codeBlock" class="size-full min-h-80"></div>
  </div>
</template>
