<script setup lang="ts">
import { nextTick, ref, useId, useTemplateRef } from 'vue'

import type { Clarification } from '@/api/client'
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import PlainTextArea from '@/components/controls/PlainTextArea.vue'
import PrimaryButton from '@/components/controls/PrimaryButton.vue'
import UserName from '@/components/users/UserName.vue'
import { useClarificationActions } from '@/composables/contests/useClarificationActions'
import { userLocation } from '@/router/locations'
import { useUserStore } from '@/stores/user'
import { formatContestDate } from '@/utils/contest'

const props = defineProps<{
  contestId: string
  question: Clarification
  canAnswer: boolean
  problemLabel: string
}>()
const session = useUserStore()
const { answer, isSaving } = useClarificationActions(() => props.contestId)
const expanded = ref(false)
const body = ref('')
const isPublic = ref(false)
const error = ref('')
const checkboxId = useId()
const formId = useId()
const buttonId = useId()
const form = useTemplateRef<HTMLFormElement>('form')
const card = useTemplateRef<HTMLElement>('card')
const setExpanded = async (open: boolean) => {
  expanded.value = open
  await nextTick()
  if (open) form.value?.querySelector('textarea')?.focus()
  else (document.getElementById(buttonId) ?? card.value)?.focus()
}
const save = async () => {
  error.value = ''
  try {
    const saved = await answer(props.question.id, body.value, isPublic.value)
    if (saved) {
      body.value = ''
      await setExpanded(false)
      card.value?.focus()
    }
  } catch (cause) {
    error.value =
      cause instanceof Error && !cause.message.startsWith('HTTP ')
        ? cause.message
        : '回答を保存できませんでした。'
  }
}
</script>

<template>
  <article ref="card" tabindex="-1" class="space-y-4 border-b border-border-primary py-6">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h3 class="min-w-0 font-semibold wrap-break-word">{{ problemLabel }}</h3>
      <span
        class="shrink-0 rounded-full bg-background-secondary px-3 py-1 text-sm text-text-secondary"
      >
        {{ question.answer === null ? '回答待ち' : '回答済み' }}
      </span>
    </div>
    <div class="flex flex-wrap gap-x-4 gap-y-1 text-sm text-text-secondary">
      <span class="min-w-0 wrap-break-word">
        <span v-if="question.authorId !== session.userId">質問者：</span>
        <RouterLink :to="userLocation(question.authorId)" class="underline">
          <span v-if="question.authorId === session.userId">あなたの質問</span>
          <UserName v-else :user-id="question.authorId" />
        </RouterLink>
      </span>
      <time :datetime="question.createdAt">{{ formatContestDate(question.createdAt) }}</time>
      <span>{{ question.isPublic ? '公開' : '非公開' }}</span>
    </div>
    <div class="space-y-2">
      <h4 class="text-sm font-semibold">質問</h4>
      <p class="wrap-break-word whitespace-pre-wrap">{{ question.body }}</p>
    </div>
    <div v-if="question.answer !== null" class="space-y-2 rounded-sm bg-background-tertiary p-4">
      <h4 class="text-sm font-semibold">回答</h4>
      <p class="wrap-break-word whitespace-pre-wrap">{{ question.answer }}</p>
    </div>
    <MonochromeButton
      v-if="canAnswer && question.answer === null && !expanded"
      :id="buttonId"
      :aria-expanded="expanded"
      :aria-controls="formId"
      small
      @click="setExpanded(true)"
      >回答する</MonochromeButton
    >
    <form
      v-if="canAnswer && expanded"
      :id="formId"
      ref="form"
      class="space-y-4"
      @submit.prevent="save"
    >
      <PlainTextArea v-model="body" label="回答" :rows="4" :disabled="isSaving" required />
      <div class="flex items-center gap-2">
        <input :id="checkboxId" v-model="isPublic" type="checkbox" :disabled="isSaving" />
        <label :for="checkboxId">質問と回答を公開する</label>
      </div>
      <p v-if="question.problemId && isPublic" class="text-sm text-text-secondary">
        この問題を含む他のコンテストの参加者にも公開されます。
      </p>
      <p class="text-sm text-text-secondary">回答後は内容・公開範囲を変更できません。</p>
      <p v-if="error" role="alert" class="text-status-error">{{ error }}</p>
      <div class="flex items-center gap-3">
        <PrimaryButton
          type="submit"
          :disabled="isSaving || !body.trim() || question.answer !== null"
          class="h-10 px-5"
          >{{ isSaving ? '保存中...' : '回答を保存' }}</PrimaryButton
        >
        <MonochromeButton type="button" :disabled="isSaving" @click="setExpanded(false)">
          閉じる
        </MonochromeButton>
      </div>
    </form>
  </article>
</template>
