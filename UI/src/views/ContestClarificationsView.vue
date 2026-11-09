<script setup lang="ts">
import { computed, nextTick, ref, useId, useTemplateRef, watch } from 'vue'

import type { Contest } from '@/api/client'
import ClarificationCard from '@/components/contests/ClarificationCard.vue'
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import CursorPagination from '@/components/controls/pagination/CursorPagination.vue'
import PlainTextArea from '@/components/controls/PlainTextArea.vue'
import PrimaryButton from '@/components/controls/PrimaryButton.vue'
import SelectBox from '@/components/controls/SelectBox.vue'
import QueryState from '@/components/QueryState.vue'
import { useClarificationActions } from '@/composables/contests/useClarificationActions'
import { useClarificationsQuery } from '@/composables/contests/useClarificationsQuery'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import { useContestId } from '@/router/locations'
import { useCursorRoute } from '@/router/useCursorRoute'
import { useUserStore } from '@/stores/user'
import type { SelectOption } from '@/types/selectOption'
import { hasContestRole } from '@/utils/contestAccess'

const id = useContestId()
const session = useUserStore()
const props = defineProps<{ contest: Contest | undefined }>()
const contest = computed(() => props.contest)
const problems = useContestProblemsQuery(contest)
const { cursor, previousCursor, changeCursor } = useCursorRoute()
const questions = useClarificationsQuery(id, cursor)
const { create, isSaving } = useClarificationActions(id)
const canAsk = computed(
  () => !!contest.value && hasContestRole(session.user, id.value, 'contestant')
)
const canAnswer = computed(() => hasContestRole(session.user, id.value, 'contest-setter'))
const options = computed(() => [
  { id: 'contest', name: 'コンテスト全体' },
  ...(problems.view.value.status === 'success'
    ? problems.view.value.data.problems.map((entry) => ({
        id: entry.problemId,
        name: `${entry.label} — ${entry.problem?.title || '問題'}`
      }))
    : [])
])
const selected = ref<SelectOption>(options.value[0])
const body = ref('')
const error = ref('')
const saved = ref(false)
const expanded = ref(false)
const form = useTemplateRef<HTMLFormElement>('form')
const formId = useId()
const buttonId = useId()
const setExpanded = async (open: boolean) => {
  expanded.value = open
  if (open) saved.value = false
  await nextTick()
  if (open) form.value?.querySelector('textarea')?.focus()
  else document.getElementById(buttonId)?.focus()
}
watch(
  () => [session.epoch, id.value],
  () => {
    body.value = ''
    expanded.value = false
    error.value = ''
    saved.value = false
    selected.value = options.value[0]!
  },
  { flush: 'sync' }
)
const submit = async () => {
  error.value = ''
  saved.value = false
  try {
    const question = await create(
      body.value,
      selected.value.id === 'contest' ? undefined : selected.value.id
    )
    if (!question) return
    body.value = ''
    saved.value = true
    await setExpanded(false)
    void changeCursor(undefined)
  } catch (cause) {
    error.value =
      cause instanceof Error && !cause.message.startsWith('HTTP ')
        ? cause.message
        : '質問を送信できませんでした。'
  }
}
</script>

<template>
  <section class="space-y-6 py-8">
    <div class="flex flex-wrap items-center justify-between gap-4">
      <h2 class="fontstyle-ui-subtitle">質問と回答</h2>
      <div class="ml-auto flex items-center gap-3">
        <MonochromeButton
          :disabled="
            !contest ||
            questions.view.value.status === 'pending' ||
            (questions.view.value.status === 'success' && questions.view.value.refreshing)
          "
          @click="questions.refresh"
          >更新</MonochromeButton
        >
        <PrimaryButton
          v-if="canAsk"
          :id="buttonId"
          class="h-10 px-5"
          :aria-expanded="expanded"
          :aria-controls="formId"
          :disabled="isSaving"
          @click="setExpanded(!expanded)"
          >質問する</PrimaryButton
        >
      </div>
    </div>
    <p v-if="saved" role="status">質問を送信しました。</p>
    <form
      v-if="canAsk && expanded"
      :id="formId"
      ref="form"
      class="space-y-4 rounded-sm border border-border-primary p-4"
      @submit.prevent="submit"
    >
      <h3 class="font-semibold">質問する</h3>
      <SelectBox v-model="selected" :options="options" label="質問の対象" :disabled="isSaving" />
      <PlainTextArea v-model="body" label="質問" :disabled="isSaving" :rows="4" required />
      <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
      <div class="flex items-center gap-3">
        <PrimaryButton type="submit" :disabled="isSaving || !body.trim()" class="h-10 px-5">{{
          isSaving ? '送信中...' : '質問を送信'
        }}</PrimaryButton>
        <MonochromeButton type="button" :disabled="isSaving" @click="setExpanded(false)">
          閉じる
        </MonochromeButton>
      </div>
    </form>
    <QueryState
      :view="contest ? questions.view.value : { status: 'pending' }"
      @retry="questions.refresh"
    >
      <template #default="{ data: page }">
        <p v-if="!page.items.length && !cursor && !page.nextCursor" class="text-text-secondary">
          質問はありません。
        </p>
        <ClarificationCard
          v-for="question in page.items"
          :key="`${session.epoch}:${id}:${question.id}`"
          :contest-id="id"
          :question="question"
          :can-answer="canAnswer"
          :problem-label="
            options.find((option) => option.id === question.problemId)?.name ??
            (question.problemId ? '問題への質問' : 'コンテスト全体')
          "
        />
        <CursorPagination
          :cursor="cursor"
          :previous-cursor="previousCursor"
          :next-cursor="page.nextCursor"
          :loading="questions.view.value.status === 'success' && questions.view.value.refreshing"
          @change="changeCursor"
        />
      </template>
    </QueryState>
  </section>
</template>
