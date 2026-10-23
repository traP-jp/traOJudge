<script setup lang="ts">
import MarkdownContent from '@/components/MarkdownContent.vue'
import SubmitForm from '@/components/SubmitForm.vue'
import { useProblemQuery } from '@/composables/problems/useProblemQuery'
import { useRouteId } from '@/router/locations'

const problemId = useRouteId()
const { view } = useProblemQuery(problemId)
</script>

<template>
  <div v-if="view.status === 'success'" class="flex w-full flex-col gap-6 py-6">
    <div class="flex flex-col gap-4">
      <h2 class="fontstyle-ui-subtitle text-text-primary">問題文</h2>
      <MarkdownContent
        :source="view.data.statement || '問題文が取得できませんでした'"
        typographer
      />
    </div>
    <hr class="border-t border-border-secondary" />
    <SubmitForm :key="problemId" :problem-id="problemId" />
  </div>
</template>
