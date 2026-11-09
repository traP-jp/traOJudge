<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useId, useTemplateRef, watch } from 'vue'

import IconDropdownTriangle from '@/components/icons/IconDropdownTriangle.vue'
import MaterialIcon from '@/components/icons/MaterialIcon.vue'
import { contestProblemLocation } from '@/router/locations'
import type { SelectOption } from '@/types/selectOption'

const props = defineProps<{
  contestId: string
  problemId: string
  problems: SelectOption[]
}>()
const currentProblem = computed(() =>
  props.problems.find((problem) => problem.id === props.problemId)
)
const container = useTemplateRef<HTMLDivElement>('container')
const trigger = useTemplateRef<HTMLButtonElement>('trigger')
const menuId = useId()
const isOpen = ref(false)
const close = (restoreFocus = false) => {
  isOpen.value = false
  if (restoreFocus) trigger.value?.focus()
}
const closeOutside = (event: Event) => {
  if (event.target instanceof Node && !container.value?.contains(event.target)) close()
}
const onKeydown = (event: KeyboardEvent) => {
  if (event.key !== 'Escape' || !isOpen.value) return
  event.preventDefault()
  close(true)
}
watch(
  () => [props.contestId, props.problemId],
  () => close()
)
onMounted(() => {
  document.addEventListener('pointerdown', closeOutside)
  document.addEventListener('focusin', closeOutside)
  document.addEventListener('keydown', onKeydown)
})
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', closeOutside)
  document.removeEventListener('focusin', closeOutside)
  document.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div ref="container" class="relative min-w-0">
    <div class="flex items-start gap-2">
      <h2 class="min-w-0 contest-page-heading wrap-break-word">
        {{ currentProblem?.name }}
      </h2>
      <button
        ref="trigger"
        type="button"
        aria-label="問題を切り替える"
        :aria-expanded="isOpen"
        :aria-controls="menuId"
        class="inline-flex size-9 shrink-0 items-center justify-center rounded-sm hover:bg-background-secondary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
        @click="isOpen = !isOpen"
      >
        <IconDropdownTriangle
          aria-hidden="true"
          class="h-dropdown-indicator"
          :class="{ 'rotate-180': !isOpen }"
        />
      </button>
    </div>
    <nav
      v-if="isOpen"
      :id="menuId"
      aria-label="問題の切り替え"
      class="absolute top-full left-0 z-20 mt-2 flex max-h-80 w-full max-w-sm flex-col gap-y-1 overflow-y-auto rounded-lg border border-border-secondary bg-background-primary p-2"
    >
      <RouterLink
        v-for="problem in problems"
        :key="problem.id"
        :to="contestProblemLocation(contestId, problem.id)"
        :aria-current="problem.id === problemId ? 'page' : undefined"
        class="flex items-start gap-2 rounded-sm px-4 py-2 fontstyle-ui-control-strong hover:bg-brand-light-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary"
        :class="
          problem.id === problemId
            ? 'bg-brand-light-primary text-brand-primary'
            : 'text-text-primary'
        "
        @click="close(true)"
      >
        <MaterialIcon
          v-if="problem.id === problemId"
          icon="check"
          size="18px"
          class="shrink-0"
          aria-hidden="true"
        />
        <span v-else class="w-4.5 shrink-0" aria-hidden="true"></span>
        <span class="min-w-0 wrap-break-word">{{ problem.name }}</span>
      </RouterLink>
    </nav>
  </div>
</template>
