<script setup lang="ts">
import { computed, ref } from 'vue'

import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import NumberTextbox from '@/components/controls/textbox/NumberTextbox.vue'
import PlainTextbox from '@/components/controls/textbox/PlainTextbox.vue'
import MaterialIcon from '@/components/icons/MaterialIcon.vue'

const props = defineProps<{ difficultyBegin: number; difficultyEnd: number }>()
const emit = defineEmits<{
  filter: [bounds: { difficultyBegin: number; difficultyEnd: number }]
}>()
const filterMenuShown = ref(false)
const toggleFilterMenu = () => {
  filterMenuShown.value = !filterMenuShown.value
}

const filterDifficultyBegin = computed({
  get: () => props.difficultyBegin,
  set: (value: number) =>
    emit('filter', { difficultyBegin: value, difficultyEnd: props.difficultyEnd })
})
const filterDifficultyEnd = computed({
  get: () => props.difficultyEnd,
  set: (value: number) =>
    emit('filter', { difficultyBegin: props.difficultyBegin, difficultyEnd: value })
})

const filterDifficultyRangeError = computed(
  () =>
    filterDifficultyBegin.value > filterDifficultyEnd.value ||
    filterDifficultyBegin.value < 1 ||
    10 < filterDifficultyBegin.value ||
    filterDifficultyEnd.value < 1 ||
    10 < filterDifficultyEnd.value
)
</script>

<template>
  <div class="relative flex items-center fontstyle-ui-subtitle text-text-primary">
    <slot />
    <MonochromeButton small @click="toggleFilterMenu">
      <span class="inline-flex items-center gap-1">
        <span>フィルタ</span>
        <MaterialIcon icon="tune" size="20px" />
      </span>
    </MonochromeButton>
    <!-- フィルタ画面 -->
    <div v-if="filterMenuShown" class="absolute top-full right-0 pt-2">
      <div
        class="flex flex-col gap-4 rounded-lg border border-border-secondary bg-background-primary p-4 text-text-secondary shadow-lg"
      >
        <div class="flex flex-col gap-2">
          <span class="fontstyle-ui-control-strong">問題名</span>
          <PlainTextbox left-icon="search" placeholder="キーワードを入力" right-icon="close" />
        </div>
        <div class="flex flex-col gap-2">
          <span class="fontstyle-ui-control-strong">難易度</span>
          <div class="flex gap-6 fontstyle-ui-control">
            <span class="flex items-center gap-2">
              <span class="w-16">
                <NumberTextbox
                  v-model="filterDifficultyBegin"
                  :min="1"
                  :max="10"
                  :error="filterDifficultyRangeError"
                />
              </span>
              <span>以上</span>
            </span>
            <span class="flex items-center gap-2">
              <span class="w-16">
                <NumberTextbox
                  v-model="filterDifficultyEnd"
                  :min="1"
                  :max="10"
                  :error="filterDifficultyRangeError"
                />
              </span>
              <span>以下</span>
            </span>
          </div>
          <div v-if="filterDifficultyRangeError" class="flex items-start gap-2 text-status-error">
            <MaterialIcon icon="error" size="1.25rem" is-filled />
            <span class="min-w-0 fontstyle-ui-control wrap-break-word">範囲が正しくありません</span>
          </div>
          <span class="fontstyle-ui-caption">難易度は1~10で表されます。</span>
        </div>
      </div>
    </div>
  </div>
</template>
