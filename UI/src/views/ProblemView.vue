<script setup lang="ts">
import SideMenuProblem from '@/components/Navigations/SideMenu/SideMenuProblem.vue'
import { ProblemsApi } from '@/api/generated'
import { ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { apiErrorMessage } from '@/utils/apiError'

const route = useRoute()
const api = new ProblemsApi()
const problemName = ref('')
const canEdit = ref(false)
const error = ref('')
watch(
  () => route.params.id,
  async (id, _, onCleanup) => {
    const controller = new AbortController()
    onCleanup(() => controller.abort())
    canEdit.value = false
    error.value = ''
    try {
      const problem = await api.getProblem({ problemId: String(id) }, { signal: controller.signal })
      if (controller.signal.aborted) return
      problemName.value = problem.title
      // 作成者の記録だけでは編集権限を持たないため，作業可能な問題一覧で確認する．
      let cursor: string | undefined
      const seen = new Set<string>()
      do {
        const page = await api.listMyProblems({ limit: 100, cursor }, { signal: controller.signal })
        if (controller.signal.aborted) return
        if (page.items.some((item) => item.id === id)) {
          canEdit.value = true
          return
        }
        cursor = page.nextCursor ?? undefined
        if (cursor && seen.has(cursor)) return
        if (cursor) seen.add(cursor)
      } while (cursor)
    } catch (cause) {
      if (!controller.signal.aborted) error.value = await apiErrorMessage(cause)
    }
  },
  { immediate: true }
)
</script>

<template>
  <div class="flex items-start gap-8 px-container-x">
    <SideMenuProblem
      :problem-name="problemName"
      :is-editorial-available="true"
      :is-admin="canEdit"
    />
    <div class="min-w-0 flex-1">
      <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
      <RouterView :key="String(route.params.id)" />
    </div>
  </div>
</template>
