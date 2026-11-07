<script setup lang="ts">
export type Column = {
  id: string
  textAlign?: 'start' | 'end' | 'center'
  width?: string
  ariaSort?: 'ascending' | 'descending'
}

// TODO: add sorting and filtering features
const {
  cols,
  rowIds,
  selectedRows,
  highlightedRowId,
  highlightedCell,
  footerLabelColspan = 1
} = defineProps<{
  cols: Column[]
  rowIds: string[]
  selectedRows?: string[]
  highlightedRowId?: string
  highlightedCell?: { rowId: string; colId: string }
  caption?: string
  rowHeaderColumn?: string
  showFooter?: boolean
  footerLabelColspan?: number
}>()
</script>

<template>
  <table class="w-full">
    <caption v-if="caption" class="sr-only">
      {{
        caption
      }}
    </caption>
    <thead class="h-10 bg-background-tertiary text-text-secondary">
      <tr>
        <th
          v-for="{ id, textAlign, width, ariaSort } in cols"
          :key="id"
          class="fontstyle-ui-control-strong border-r border-border-secondary px-3 py-2 last:border-r-0"
          :style="{ textAlign, width }"
          :aria-sort="ariaSort"
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
          '!border-brand-primary bg-brand-light-primary': selectedRows?.includes(rowId),
          'bg-brand-light-primary': highlightedRowId === rowId
        }"
      >
        <template v-for="col in cols" :key="col.id">
          <th
            v-if="!rowHeaderColumn || col.id === rowHeaderColumn"
            :scope="rowHeaderColumn ? 'row' : undefined"
            class="fontstyle-ui-body h-12 px-3"
            :style="{ textAlign: col.textAlign, width: col.width }"
          >
            <slot name="cell" :row-id="rowId" :col-id="col.id" />
          </th>
          <td
            v-else
            :id="
              highlightedCell?.rowId === rowId && highlightedCell.colId === col.id
                ? 'highlighted-table-cell'
                : undefined
            "
            :tabindex="
              highlightedCell?.rowId === rowId && highlightedCell.colId === col.id ? -1 : undefined
            "
            class="fontstyle-ui-body h-12 px-3"
            :class="{
              'relative z-10 bg-brand-light-primary':
                highlightedCell?.rowId === rowId && highlightedCell.colId === col.id
            }"
            :style="{ textAlign: col.textAlign, width: col.width }"
          >
            <slot name="cell" :row-id="rowId" :col-id="col.id" />
            <span
              v-if="highlightedCell?.rowId === rowId && highlightedCell.colId === col.id"
              aria-hidden="true"
              class="pointer-events-none absolute inset-0 border-2 border-brand-primary"
            />
          </td>
        </template>
      </tr>
    </tbody>
    <tfoot v-if="showFooter" class="border-t-2 border-border-primary bg-background-secondary">
      <tr>
        <template v-for="(col, index) in cols" :key="col.id">
          <th
            v-if="index === 0 && footerLabelColspan > 1"
            scope="row"
            :colspan="footerLabelColspan"
            class="fontstyle-ui-body h-12 border-r border-border-secondary px-3 text-center"
          >
            <slot name="foot" :col-id="col.id" />
          </th>
          <template v-else-if="footerLabelColspan === 1 || index >= footerLabelColspan">
            <th
              v-if="col.id === rowHeaderColumn"
              scope="row"
              class="fontstyle-ui-body h-12 px-3"
              :style="{ textAlign: col.textAlign, width: col.width }"
            >
              <slot name="foot" :col-id="col.id" />
            </th>
            <td
              v-else
              class="fontstyle-ui-body h-12 px-3"
              :style="{ textAlign: col.textAlign, width: col.width }"
            >
              <slot name="foot" :col-id="col.id" />
            </td>
          </template>
        </template>
      </tr>
    </tfoot>
  </table>
</template>
