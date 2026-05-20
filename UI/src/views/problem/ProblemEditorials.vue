<script setup lang="ts">
import EditorialsList from '@/components/EditorialsList.vue'
import { useRoute } from 'vue-router'

const route = useRoute()
const problemId = route.params.id as string
</script>

<template>
  <div class="flex flex-1 flex-col items-start py-6">
    <div class="flex flex-col items-center gap-4 self-stretch">
      <div class="fontstyle-ui-subtitle flex items-start self-stretch text-text-primary">
        解説一覧
      </div>
      <EditorialsList :problem-id="problemId" />
    </div>
  </div>
</template>
