<script setup lang="ts">
import { useUserQuery } from '@/composables/users/useUserQuery'

const props = defineProps<{ userId: string }>()
const { view } = useUserQuery(() => props.userId)
</script>

<template>
  <span>{{
    view.status === 'success'
      ? view.data.displayName
      : view.status === 'pending'
        ? '読み込み中...'
        : '不明なユーザー'
  }}</span>
</template>
