<script setup lang="ts">
import ProblemSubmitForm from '@/components/submissions/ProblemSubmitForm.vue'
import { useRouteId } from '@/router/locations'
const problemId = useRouteId()
</script>

<template>
  <div class="flex w-full flex-col gap-6 py-6">
    <ProblemSubmitForm :key="problemId" :problem-id="problemId" />
  </div>
</template>
