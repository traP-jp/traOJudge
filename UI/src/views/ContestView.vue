<script setup lang="ts">
import ContestHeader from '@/components/contests/ContestHeader.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import QueryState from '@/components/QueryState.vue'
import { useContestQuery } from '@/composables/contests/useContestQuery'
import { useContestId } from '@/router/locations'
import { formatContestDate, formatContestDuration } from '@/utils/contest'

const { view, refresh } = useContestQuery(useContestId())
</script>

<template>
  <div class="mx-auto min-h-header-offset max-w-7xl px-4 py-8 font-primary sm:px-8 lg:px-12">
    <QueryState :view="view" @retry="refresh">
      <template #default="{ data: contest }">
        <ContestHeader :contest="contest" />
        <div class="flex min-w-0 flex-col gap-10 py-8 lg:flex-row lg:gap-12">
          <article aria-labelledby="contest-overview" class="min-w-0 flex-1">
            <h2 id="contest-overview" class="mb-6 fontstyle-ui-subtitle">コンテスト概要</h2>
            <MarkdownContent v-if="contest.description?.trim()" :source="contest.description" />
            <p v-else class="leading-7 text-text-secondary">
              コンテストの概要はまだ掲載されていません．
            </p>
          </article>

          <aside aria-label="開催情報とルール" class="w-full shrink-0 lg:w-72">
            <section
              aria-labelledby="contest-schedule"
              class="border-t border-border-primary pt-6 lg:border-t-0 lg:pt-0"
            >
              <h2 id="contest-schedule" class="mb-5 fontstyle-ui-subtitle">開催情報</h2>
              <dl class="space-y-5 text-sm">
                <div>
                  <dt class="mb-1 text-text-secondary">開始日時</dt>
                  <dd class="font-medium text-text-primary tabular-nums">
                    <time :datetime="contest.startsAt">{{
                      formatContestDate(contest.startsAt)
                    }}</time>
                  </dd>
                </div>
                <div>
                  <dt class="mb-1 text-text-secondary">終了日時</dt>
                  <dd class="font-medium text-text-primary tabular-nums">
                    <time :datetime="contest.endsAt">{{ formatContestDate(contest.endsAt) }}</time>
                  </dd>
                </div>
                <div>
                  <dt class="mb-1 text-text-secondary">開催時間</dt>
                  <dd class="font-medium text-text-primary tabular-nums">
                    {{
                      formatContestDuration(
                        Date.parse(contest.endsAt) - Date.parse(contest.startsAt)
                      )
                    }}
                  </dd>
                </div>
              </dl>
              <p class="mt-4 text-xs text-text-secondary">日時は日本標準時（JST）です</p>
            </section>

            <section
              aria-labelledby="contest-rules"
              class="mt-8 border-t border-border-primary pt-6"
            >
              <h2 id="contest-rules" class="mb-5 fontstyle-ui-subtitle">ルール</h2>
              <dl class="space-y-5 text-sm">
                <div>
                  <dt class="mb-1 text-text-secondary">同点時の順位</dt>
                  <dd class="font-medium text-text-primary">
                    {{ contest.rules.tieBreak === 'icpc' ? 'ICPC方式' : 'AtCoder方式' }}
                  </dd>
                </div>
                <div>
                  <dt class="mb-1 text-text-secondary">誤答ペナルティ</dt>
                  <dd class="font-medium text-text-primary tabular-nums">
                    {{
                      contest.rules.penaltyMinutes === 0
                        ? 'なし'
                        : `${contest.rules.penaltyMinutes}分`
                    }}
                  </dd>
                </div>
              </dl>
            </section>
          </aside>
        </div>
      </template>
    </QueryState>
  </div>
</template>
