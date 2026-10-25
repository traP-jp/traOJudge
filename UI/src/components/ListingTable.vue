<script setup lang="ts">
import type { Column } from '@/types/listingTable'

// TODO: add sorting and filtering features
const { cols, rowIds, selectedRows } = defineProps<{
  cols: Column[]
  rowIds: string[]
  selectedRows?: string[]
}>()
</script>

<template>
  <table class="w-full">
    <thead class="h-10 bg-background-tertiary text-text-secondary">
      <tr>
        <th
          v-for="{ id, textAlign, width } in cols"
          :key="id"
          class="border-r border-border-secondary px-3 py-2 fontstyle-ui-control-strong last:border-r-0"
          :style="{ textAlign, width }"
        >
          <slot name="head" :col-id="id" />
        </th>
      </tr>
    </thead>
    <tbody>
      <tr
        v-for="rowId in rowIds"
        :key="rowId"
        class="h-9 border border-solid border-border-secondary border-x-transparent border-t-transparent text-text-primary"
        :class="{
          'border-brand-primary! bg-brand-light-primary': selectedRows?.includes(rowId)
        }"
      >
        <th
          v-for="col in cols"
          :key="col.id"
          class="h-12 px-3 fontstyle-ui-body"
          :style="{ textAlign: col.textAlign, width: col.width }"
        >
          <slot name="cell" :row-id="rowId" :col-id="col.id" />
        </th>
      </tr>
    </tbody>
  </table>
</template>
