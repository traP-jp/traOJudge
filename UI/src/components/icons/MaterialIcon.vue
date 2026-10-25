<script setup lang="ts">
import type { Icon } from '@/types/icon'

const { size = '1.5rem', isFilled = false } = defineProps<{
  icon: Icon
  size?: string
  isFilled?: boolean
}>()
</script>

<template>
  <!-- eslint-disable tailwindcss/no-custom-classname -- material-symbols-rounded は Google Material Symbols フォントのクラス。アイコン表示の挙動 (FILL バリエーション軸) は CSS font-variation-settings 経由で制御するため Tailwind 管轄外。 -->
  <span
    class="material-symbols-rounded"
    :style="{ fontVariationSettings: '\'FILL\' ' + (isFilled ? 1 : 0), fontSize: size }"
  >
    {{ icon }}
  </span>
  <!-- eslint-enable tailwindcss/no-custom-classname -->
</template>
