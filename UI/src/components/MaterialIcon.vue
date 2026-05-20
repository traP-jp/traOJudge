<script setup lang="ts">
export type Icon =
  | 'add'
  | 'alternate_email'
  | 'arrow_left_alt'
  | 'assignment'
  | 'assignment_turned_in'
  | 'check'
  | 'close'
  | 'code'
  | 'content_copy'
  | 'delete'
  | 'edit'
  | 'edit_note'
  | 'error'
  | 'format_list_bulleted'
  | 'id_card'
  | 'info'
  | 'keyboard_arrow_down'
  | 'labs'
  | 'library_books'
  | 'logout'
  | 'more_vert'
  | 'open_in_new'
  | 'person'
  | 'save'
  | 'search'
  | 'send'
  | 'settings'
  | 'star_half'
  | 'star_rate'
  | 'tune'
  | 'visibility'
  | 'visibility_off'
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
