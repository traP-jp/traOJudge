<script setup lang="ts">
import type { Contest } from '@/api/client'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { formatContestDate, formatContestDuration } from '@/utils/contest'

defineProps<{ contest: Contest | undefined }>()
</script>

<template>
  <div class="flex min-w-0 flex-col gap-10 py-6 sm:py-8 lg:flex-row lg:gap-12">
    <article aria-labelledby="contest-overview" class="min-w-0 flex-1">
      <h2 id="contest-overview" class="mb-6 contest-page-heading">コンテスト概要</h2>
      <p v-if="!contest" role="status" class="py-8 text-text-secondary">読み込み中...</p>
      <MarkdownContent
        v-else-if="contest.description?.trim()"
        :source="contest.description"
        :heading-level="3"
        class="contest-markdown"
      />
      <p v-else class="leading-7 text-text-secondary">コンテストの概要はまだ掲載されていません．</p>
    </article>

    <aside aria-label="開催情報とルール" class="w-full shrink-0 lg:w-72">
      <section
        aria-labelledby="contest-schedule"
        class="border-t border-border-primary pt-6 lg:border-t-0 lg:pt-0"
      >
        <h2 id="contest-schedule" class="mb-5 contest-page-heading">開催情報</h2>
        <dl v-if="contest" class="space-y-5 text-sm">
          <div>
            <dt class="mb-1 text-text-secondary">開始日時</dt>
            <dd class="font-medium text-text-primary tabular-nums">
              <time :datetime="contest.startsAt">{{ formatContestDate(contest.startsAt) }}</time>
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
              {{ formatContestDuration(Date.parse(contest.endsAt) - Date.parse(contest.startsAt)) }}
            </dd>
          </div>
        </dl>
        <p v-else class="py-8 text-text-secondary">読み込み中...</p>
        <p v-if="contest" class="mt-4 text-xs text-text-secondary">日時は日本標準時（JST）です</p>
      </section>

      <section aria-labelledby="contest-rules" class="mt-8 border-t border-border-primary pt-6">
        <h2 id="contest-rules" class="mb-5 contest-page-heading">ルール</h2>
        <dl v-if="contest" class="space-y-5 text-sm">
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
                contest.rules.penaltyMinutes === 0 ? 'なし' : `${contest.rules.penaltyMinutes}分`
              }}
            </dd>
          </div>
        </dl>
        <p v-else class="py-8 text-text-secondary">読み込み中...</p>
      </section>
    </aside>
  </div>
</template>
