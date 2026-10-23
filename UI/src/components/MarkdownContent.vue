<script setup lang="ts">
import { computed } from 'vue'
import MarkdownIt from 'markdown-it'

const { source, typographer = false } = defineProps<{ source: string; typographer?: boolean }>()
const markdown = new MarkdownIt({ html: false, linkify: true })
const renderedContent = computed(() => markdown.set({ typographer }).render(source))
</script>

<template>
  <!-- eslint-disable vue/no-v-html -- markdown-itでraw HTMLを無効にし，危険なURLも既定の検証で拒否する． -->
  <div
    class="min-w-0 leading-7 wrap-break-word text-text-primary *:first:mt-0 *:last:mb-0 [&_a]:text-brand-primary [&_a]:underline [&_a]:underline-offset-2 [&_a:focus-visible]:outline-2 [&_a:focus-visible]:outline-offset-2 [&_a:focus-visible]:outline-brand-primary [&_a:hover]:text-brand-secondary [&_blockquote]:my-4 [&_blockquote]:border-l-4 [&_blockquote]:border-border-primary [&_blockquote]:pl-4 [&_blockquote]:text-text-secondary [&_code]:rounded-sm [&_code]:bg-background-tertiary [&_code]:px-1 [&_code]:text-sm [&_h1]:mt-8 [&_h1]:mb-4 [&_h1]:text-2xl [&_h1]:font-semibold [&_h2]:mt-8 [&_h2]:mb-3 [&_h2]:text-xl [&_h2]:font-semibold [&_h3]:mt-6 [&_h3]:mb-2 [&_h3]:text-lg [&_h3]:font-semibold [&_h4]:mt-4 [&_h4]:mb-2 [&_h4]:font-semibold [&_hr]:my-8 [&_hr]:border-border-secondary [&_img]:h-auto [&_img]:max-w-full [&_li]:my-1 [&_ol]:my-4 [&_ol]:list-decimal [&_ol]:pl-6 [&_p]:my-4 [&_pre]:my-4 [&_pre]:overflow-x-auto [&_pre]:rounded-sm [&_pre]:bg-background-tertiary [&_pre]:p-4 [&_pre_code]:p-0 [&_table]:my-4 [&_table]:block [&_table]:max-w-full [&_table]:border-collapse [&_table]:overflow-x-auto [&_td]:border [&_td]:border-border-primary [&_td]:px-3 [&_td]:py-2 [&_th]:border [&_th]:border-border-primary [&_th]:bg-background-tertiary [&_th]:px-3 [&_th]:py-2 [&_th]:text-left [&_ul]:my-4 [&_ul]:list-disc [&_ul]:pl-6"
    v-html="renderedContent"
  />
  <!-- eslint-enable vue/no-v-html -->
</template>
