<script setup lang="ts">
import { computed, ref } from 'vue'

import PlainTextbox from '@/components/controls/textbox/PlainTextbox.vue'
import type { Icon } from '@/types/icon'

const visible = ref<boolean>(false)
const rightIcon = computed<Icon>(() => (visible.value ? 'visibility_off' : 'visibility'))
const toggleVisibility = () => (visible.value = !visible.value)
</script>

<template>
  <PlainTextbox
    displays-length
    :right-icon="rightIcon"
    :type="visible ? 'text' : 'password'"
    class="h-5.5"
    @click-right="toggleVisibility"
  />
</template>
