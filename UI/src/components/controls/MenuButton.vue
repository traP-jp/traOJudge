<script setup lang="ts">
import MaterialIcon from '@/components/icons/MaterialIcon.vue'
import type { Icon } from '@/types/icon'

const { selected = false } = defineProps<{
  icon?: Icon
  selected?: boolean
}>()
const emit = defineEmits<{ (e: 'click'): void }>()

function onClick() {
  emit('click')
}
</script>

<template>
  <button
    class="flex w-full items-center gap-2 rounded-sm px-4 py-1 hover:bg-brand-light-primary"
    :class="selected ? 'bg-brand-light-primary text-brand-primary' : 'text-text-primary'"
    @click="onClick"
  >
    <MaterialIcon v-if="icon" :icon="icon" :is-filled="selected" size="18px" />
    <span v-else class="w-4.5"></span>
    <span class="inline-block fontstyle-ui-control-strong">
      <slot />
    </span>
  </button>
</template>
