<script setup lang="ts">
import { useRoute } from 'vue-router'
import SubmissionsTable from '@/components/SubmissionsTable.vue'
const route = useRoute()
</script>
<template>
  <section class="flex w-full flex-col gap-4 py-6">
    <h2 class="fontstyle-ui-subtitle">この問題への自分の提出</h2>
    <SubmissionsTable :problem-id="String(route.params.id)" />
  </section>
</template>
