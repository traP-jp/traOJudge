<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import SideMenuProblemEdit from '@/components/problems/SideMenuProblemEdit.vue'

const route = useRoute()

if (typeof route.params.id !== 'string') throw new Error('Invalid route')
const problemId = ref<string>('')
watch(
  () => route.params.id,
  (id) => {
    problemId.value = String(id)
  },
  { immediate: true }
)
</script>

<template>
  <div class="flex gap-8 px-container-x">
    <nav class="sticky top-14 h-header-offset">
      <SideMenuProblemEdit :problem-id="problemId" />
    </nav>
    <main class="min-w-0 flex-auto py-6">
      <RouterView />
    </main>
  </div>
</template>
