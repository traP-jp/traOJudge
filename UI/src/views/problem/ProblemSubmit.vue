<script setup lang="ts">
import SubmitForm from '@/components/SubmitForm.vue'
import { useRouteId } from '@/router/locations'
const problemId = useRouteId()
</script>

<template>
  <div class="flex w-full flex-col gap-6 py-6">
    <SubmitForm :key="problemId" :problem-id="problemId" />
  </div>
</template>
