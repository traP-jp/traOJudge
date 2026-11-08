<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import type { Contest } from '@/api/client'
import PrimaryButton from '@/components/controls/PrimaryButton.vue'
import SelectBox from '@/components/controls/SelectBox.vue'
import CodeEditor from '@/components/editor/CodeEditor.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestPhase } from '@/composables/contests/useContestPhase'
import { useContestSubmission } from '@/composables/submissions/useContestSubmission'
import { useSubmissionLanguages } from '@/composables/submissions/useSubmissionLanguages'
import { useUserStore } from '@/stores/user'
import type { SelectOption } from '@/types/selectOption'
import { hasContestRole } from '@/utils/contestAccess'

const props = defineProps<{ contest: Contest; problems: SelectOption[] }>()
const session = useUserStore()
const route = useRoute()
const router = useRouter()
const { phase } = useContestPhase(() => props.contest)
const participant = computed(() => hasContestRole(session.user, props.contest.id, 'contestant'))
const problemId = computed(() =>
  typeof route.query.problemId === 'string' ? route.query.problemId : (props.problems[0]?.id ?? '')
)
const selectedProblem = computed({
  get: () => props.problems.find((problem) => problem.id === problemId.value),
  set: (problem: SelectOption | undefined) => {
    if (problem) void router.replace({ query: { ...route.query, problemId: problem.id } })
  }
})
type Draft = { source: string | null; languageId: string }
const drafts = reactive<Record<string, Draft>>({})
const error = ref('')
const submitted = ref(false)
watch(
  problemId,
  (id) => {
    drafts[id] ??= { source: '', languageId: '' }
    error.value = ''
    submitted.value = false
  },
  { immediate: true, flush: 'sync' }
)
watch(
  () => session.epoch,
  () => {
    for (const key of Object.keys(drafts)) delete drafts[key]
    drafts[problemId.value] = { source: '', languageId: '' }
    error.value = ''
    submitted.value = false
  },
  { flush: 'sync' }
)
const draft = computed(() => drafts[problemId.value])
const languages = useSubmissionLanguages()
const availableLanguages = computed(() =>
  languages.view.value.status === 'success' ? languages.view.value.data : []
)
const selectedLanguage = computed({
  get: () =>
    availableLanguages.value.find((language) => language.id === draft.value?.languageId) ??
    availableLanguages.value[0],
  set: (language: SelectOption | undefined) => {
    if (draft.value && language) draft.value.languageId = language.id
  }
})
const { submit, isSubmitting } = useContestSubmission(() => ({
  contestId: props.contest.id,
  problemId: problemId.value
}))
const handleSubmit = async () => {
  if (
    !draft.value ||
    !selectedProblem.value ||
    !selectedLanguage.value ||
    phase.value !== 'running'
  )
    return
  error.value = ''
  submitted.value = false
  try {
    const accepted = await submit({
      source: draft.value.source ?? '',
      // shortcut: ファイル名は暫定固定。バックエンドの命名規則に合わせる際に見直す。
      filename: 'main.cpp',
      languageId: selectedLanguage.value.id
    })
    if (accepted) submitted.value = true
  } catch (cause) {
    error.value =
      cause instanceof Error && !cause.message.startsWith('HTTP ')
        ? cause.message
        : '提出できませんでした。'
  }
}
</script>

<template>
  <p v-if="!participant" class="py-6 text-text-secondary">
    提出するには、このコンテストへの参加が必要です。
  </p>
  <div v-else class="space-y-6 py-6">
    <p v-if="phase === 'upcoming'" class="text-text-secondary">提出受付前です。</p>
    <p v-else-if="phase === 'finished'" class="text-text-secondary">提出受付は終了しました。</p>
    <SelectBox
      v-model="selectedProblem"
      :options="problems"
      label="問題"
      :disabled="isSubmitting"
      required
    />
    <p v-if="!selectedProblem" role="alert" class="text-status-error">
      提出する問題を選択してください。
    </p>
    <QueryState :view="languages.view.value" @retry="languages.refresh">
      <template #default="{ data }">
        <p v-if="data.length === 0" class="text-text-secondary">利用できる言語がありません。</p>
        <form
          v-else-if="draft && selectedLanguage"
          class="space-y-5"
          @submit.prevent="handleSubmit"
        >
          <SelectBox
            v-model="selectedLanguage"
            :options="data"
            label="言語"
            :disabled="isSubmitting"
            required
          />
          <p class="text-sm text-text-secondary">{{ selectedLanguage.versionLabel }}</p>
          <CodeEditor v-model="draft.source" :language="selectedLanguage" />
          <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
          <p v-if="submitted" role="status">提出を受け付けました。</p>
          <PrimaryButton
            type="submit"
            :disabled="
              isSubmitting || phase !== 'running' || !selectedProblem || !draft.source?.trim()
            "
            class="h-12 w-32"
          >
            {{ isSubmitting ? '提出中...' : '提出' }}
          </PrimaryButton>
        </form>
      </template>
    </QueryState>
  </div>
</template>
