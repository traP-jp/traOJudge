<script setup lang="ts">
import { useRoute } from 'vue-router'

import EditorialsList from '@/components/problems/EditorialsList.vue'

const route = useRoute()
const problemId = route.params.id as string
</script>

<template>
  <div class="flex flex-1 flex-col items-start py-6">
    <div class="flex flex-col items-center gap-4 self-stretch">
      <div class="flex items-start self-stretch fontstyle-ui-subtitle text-text-primary">
        解説一覧
      </div>
      <EditorialsList :problem-id="problemId" />
    </div>
  </div>
</template>
