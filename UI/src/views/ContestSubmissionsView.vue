<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import type { Contest } from '@/api/client'
import MonochromeButton from '@/components/controls/MonochromeButton.vue'
import CursorPagination from '@/components/controls/pagination/CursorPagination.vue'
import SelectBox from '@/components/controls/SelectBox.vue'
import QueryState from '@/components/QueryState.vue'
import JudgeRunResult from '@/components/submissions/JudgeRunResult.vue'
import { useContestPhase } from '@/composables/contests/useContestPhase'
import { useContestProblemsQuery } from '@/composables/contests/useContestProblemsQuery'
import {
  type ContestSubmissionScope,
  useContestSubmissionsQuery
} from '@/composables/submissions/useContestSubmissionsQuery'
import { useSubmissionLanguages } from '@/composables/submissions/useSubmissionLanguages'
import { contestProblemLocation, useContestId } from '@/router/locations'
import { useCursorRoute } from '@/router/useCursorRoute'
import { useUserStore } from '@/stores/user'
import type { SelectOption } from '@/types/selectOption'
import { formatContestDate } from '@/utils/contest'
import { hasContestRole } from '@/utils/contestAccess'

const id = useContestId()
const route = useRoute()
const router = useRouter()
const session = useUserStore()
const props = defineProps<{ contest: Contest | undefined }>()
const contest = computed(() => props.contest)
const { phase } = useContestPhase(contest)
const problems = useContestProblemsQuery(contest)
const languages = useSubmissionLanguages()
const setter = computed(() => hasContestRole(session.user, id.value, 'contest-setter'))
const scope = computed<ContestSubmissionScope>(() => {
  if (route.query.scope === 'all' || route.query.scope === 'mine') return route.query.scope
  if (route.query.scope === 'team' && contest.value?.participationMode === 'team') return 'team'
  return session.isAuthenticated && !setter.value ? 'mine' : 'all'
})
watch(
  () => [session.isInitialized, contest.value, route.query.scope, route.query.problemId],
  () => {
    if (!session.isInitialized || !contest.value) return
    if (route.query.scope === scope.value && route.query.problemId === undefined) return
    void router.replace({
      query: {
        ...route.query,
        scope: scope.value,
        problemId: undefined,
        cursor:
          route.query.scope === undefined || route.query.scope === scope.value
            ? route.query.cursor
            : undefined
      },
      hash: route.hash
    })
  },
  { immediate: true }
)
const teamId = computed(() => (typeof route.query.teamId === 'string' ? route.query.teamId : ''))
const { cursor, previousCursor, changeCursor } = useCursorRoute()
const canRead = computed(() => {
  if (scope.value === 'all') return phase.value === 'finished' || setter.value
  if (!session.isAuthenticated) return false
  return scope.value === 'mine' || (contest.value?.participationMode === 'team' && !!teamId.value)
})
const submissions = useContestSubmissionsQuery(() => ({
  contestId: id.value,
  scope: scope.value,
  teamId: teamId.value,
  cursor: cursor.value,
  enabled: !!contest.value && canRead.value
}))
const ownTeam = computed(() => {
  if (teamId.value) return teamId.value
  if (submissions.view.value.status !== 'success') return ''
  return (
    submissions.view.value.data.items.find((item) => item.userId === session.userId && item.teamId)
      ?.teamId ?? ''
  )
})
const scopeOptions = computed<SelectOption[]>(() => [
  { id: 'all', name: '全員' },
  { id: 'mine', name: '自分' },
  ...(contest.value?.participationMode === 'team' ? [{ id: 'team', name: '自分のチーム' }] : [])
])
const selectedScope = computed({
  get: () => scopeOptions.value.find((option) => option.id === scope.value),
  set: (option: SelectOption | undefined) => {
    if (!option || option.id === scope.value) return
    const target = {
      path: route.path,
      query: {
        ...route.query,
        scope: option.id,
        cursor: undefined,
        problemId: undefined,
        teamId: ownTeam.value || undefined
      },
      hash: route.hash
    }
    void router.push(
      option.id !== 'all' && !session.isAuthenticated
        ? { path: '/login', query: { redirect: router.resolve(target).fullPath } }
        : target
    )
  }
})
const updating = computed(
  () =>
    submissions.view.value.status === 'pending' ||
    (submissions.view.value.status === 'success' && submissions.view.value.refreshing)
)
const problemLabels = computed(() =>
  problems.view.value.status === 'success'
    ? problems.view.value.data.problems.map((entry) => ({
        id: entry.problemId,
        name: `${entry.label} — ${entry.problem?.title || '問題'}`
      }))
    : []
)
</script>

<template>
  <section class="space-y-6 py-8">
    <h2 class="fontstyle-ui-subtitle">提出一覧</h2>
    <div class="flex flex-wrap items-end gap-4">
      <SelectBox
        v-model="selectedScope"
        :options="scopeOptions"
        label="提出者"
        container-class="min-w-48 flex-1 sm:max-w-64"
        :disabled="session.isLoading"
      />
      <MonochromeButton
        v-if="canRead"
        small
        class="ml-auto"
        :disabled="updating"
        @click="submissions.refresh"
        >{{ updating ? '更新中…' : '更新' }}</MonochromeButton
      >
    </div>
    <QueryState v-if="!contest" :view="{ status: 'pending' }" />
    <template v-else-if="!canRead">
      <p v-if="scope === 'all'" class="text-text-secondary">コンテスト終了後に公開されます。</p>
      <p v-else-if="!session.isAuthenticated">
        <RouterLink
          :to="{ path: '/login', query: { redirect: route.fullPath } }"
          class="text-brand-primary underline"
          >ログイン</RouterLink
        >して提出を確認してください。
      </p>
      <p v-else class="text-text-secondary">
        所属チームを確認できません。
        <RouterLink :to="{ query: { scope: 'mine' } }" class="text-brand-primary underline"
          >自分の提出</RouterLink
        >
      </p>
    </template>
    <template v-else>
      <QueryState :view="submissions.view.value" @retry="submissions.refresh">
        <template #default="{ data: page }">
          <div class="overflow-x-auto">
            <table class="w-full min-w-3xl text-left text-sm">
              <caption class="sr-only">
                コンテストの提出一覧
              </caption>
              <thead class="border-b border-border-primary text-text-secondary">
                <tr>
                  <th scope="col" class="p-3">提出日時</th>
                  <th scope="col" class="p-3">問題</th>
                  <th scope="col" class="p-3">提出者</th>
                  <th scope="col" class="p-3">言語</th>
                  <th scope="col" class="p-3">判定</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="item in page.items"
                  :key="`${session.epoch}:${scope}:${teamId}:${item.id}`"
                  class="border-b border-border-primary align-top"
                >
                  <td class="p-3 whitespace-nowrap">
                    <time :datetime="item.submittedAt">{{
                      formatContestDate(item.submittedAt)
                    }}</time>
                  </td>
                  <td class="p-3">
                    <RouterLink
                      :to="contestProblemLocation(id, item.problemId)"
                      class="text-brand-primary underline"
                      >{{
                        problemLabels.find((option) => option.id === item.problemId)?.name ??
                        item.problemId
                      }}</RouterLink
                    >
                  </td>
                  <td class="max-w-48 p-3 wrap-break-word">
                    {{ item.userId === session.userId ? '自分' : item.userId }}
                    <RouterLink
                      v-if="item.userId === session.userId && item.teamId"
                      :to="{ query: { scope: 'team', teamId: item.teamId } }"
                      class="block text-brand-primary underline"
                      >チームの提出</RouterLink
                    >
                  </td>
                  <td class="p-3">
                    {{
                      languages.view.value.status === 'success'
                        ? (languages.view.value.data.find(
                            (language) => language.id === item.languageId
                          )?.name ?? item.languageId)
                        : item.languageId
                    }}
                  </td>
                  <td class="min-w-48 p-3">
                    <JudgeRunResult
                      :submission-id="item.id"
                      :judge-run-id="item.currentJudgeRunId"
                      :can-read="
                        session.isAuthenticated &&
                        (setter || item.userId === session.userId || scope === 'team')
                      "
                    />
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          <p
            v-if="!page.items.length && !cursor && !page.nextCursor"
            class="py-4 text-text-secondary"
          >
            提出はありません。
          </p>
          <CursorPagination
            :cursor="cursor"
            :previous-cursor="previousCursor"
            :next-cursor="page.nextCursor"
            :loading="
              submissions.view.value.status === 'success' && submissions.view.value.refreshing
            "
            @change="changeCursor"
          />
        </template>
      </QueryState>
    </template>
  </section>
</template>
