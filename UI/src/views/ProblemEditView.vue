<script setup lang="ts">
import { useRoute } from 'vue-router'
import SideMenuProblemEdit from '@/components/problems/SideMenuProblemEdit.vue'
import { ref, watch } from 'vue'

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
    <main class="flex-auto py-6">
      <RouterView />
    </main>
  </div>
</template>
