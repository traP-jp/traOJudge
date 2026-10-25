<script setup lang="ts">
import { computed } from 'vue'
import 'katex/dist/katex.min.css'
import { useMarkdownRenderer } from '@/composables/useMarkdownRenderer'

const { source } = defineProps<{
  source: string
}>()

const { renderMarkdown } = useMarkdownRenderer()

const rendered = computed(() => renderMarkdown(source))
</script>

<template>
  <!-- eslint-disable vue/no-v-html -- useMarkdownRenderer 内で DOMPurify によりサニタイズ済み． -->
  <div
    class="prose prose-sm min-w-0 max-w-none break-words text-text-primary prose-headings:text-text-primary prose-a:text-brand-primary prose-strong:text-text-primary prose-code:text-text-primary prose-pre:bg-background-tertiary prose-pre:text-text-primary [&_.katex-display]:overflow-x-auto [&_.katex-display]:overflow-y-hidden [&_table]:block [&_table]:overflow-x-auto"
    v-html="rendered"
  ></div>
  <!-- eslint-enable vue/no-v-html -->
</template>
