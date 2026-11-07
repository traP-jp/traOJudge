<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { ProblemsApi, type Testcase } from '@/api/generated'
import { apiErrorMessage } from '@/utils/apiError'
const route = useRoute()
const testcases = ref<Testcase[]>([])
const loading = ref(true)
const error = ref('')
watch(
  () => route.params.id,
  async (id) => {
    loading.value = true
    error.value = ''
    try {
      testcases.value = (await new ProblemsApi().listTestcases({ problemId: String(id) })).items
    } catch (cause) {
      error.value = await apiErrorMessage(cause)
    } finally {
      loading.value = false
    }
  },
  { immediate: true }
)
</script>
<template>
  <div class="flex w-full flex-col gap-4">
    <h2 class="fontstyle-ui-subtitle">テストケース</h2>
    <p class="text-text-secondary">テストケースの追加・編集・削除は準備中です．</p>
    <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
    <p v-else-if="loading">読み込み中...</p>
    <p v-else-if="!testcases.length">テストケースはありません．</p>
    <ul v-else class="flex flex-col gap-3">
      <li
        v-for="testcase in testcases"
        :key="testcase.id"
        class="rounded border border-border-primary p-4"
      >
        <p>{{ testcase.name }}</p>
        <p class="text-text-secondary">
          入力ファイル登録済み・期待出力{{ testcase.expectedOutputFileId ? '登録済み' : 'なし' }}
        </p>
      </li>
    </ul>
  </div>
</template>
