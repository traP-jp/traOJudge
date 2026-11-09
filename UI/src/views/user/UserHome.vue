<script setup lang="ts">
import { computed } from 'vue'

import type { Profile } from '@/api/client'
import TextLink from '@/components/TextLink.vue'

const { user } = defineProps<{ userId: string; user: Profile | null }>()

/**
 * GitHub username validation
 * - Alphanumeric characters and hyphens only
 * - Cannot start or end with a hyphen
 * - No consecutive hyphens
 * - 1-39 characters
 * Source: https://github.com/shinnn/github-username-regex
 *         https://docs.github.com/en/get-started/signing-up-for-github/signing-up-for-a-new-github-account
 */
const GITHUB_USERNAME_REGEX = /^[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,37}[a-zA-Z0-9])?$/

/**
 * X (Twitter) username validation
 * - Alphanumeric characters and underscores only
 * - 1-15 characters
 * Source: https://help.x.com/en/managing-your-account/x-username-rules
 */
const X_USERNAME_REGEX = /^[A-Za-z0-9_]{1,15}$/

const isValidGithubUsername = (username: string): boolean => {
  if (!username || username.includes('--')) return false
  return GITHUB_USERNAME_REGEX.test(username)
}

const isValidXUsername = (username: string): boolean => {
  if (!username) return false
  return X_USERNAME_REGEX.test(username)
}

const atcoderId = computed(() => user?.atcoderId ?? '')
const githubId = computed(() => user?.githubId ?? '')
const xId = computed(() => user?.xId ?? '')
const selfIntroduction = computed(() => user?.bio ?? '')

const isGithubIdValid = computed(() => isValidGithubUsername(githubId.value))
const isXIdValid = computed(() => isValidXUsername(xId.value))
const isAtcoderIdValid = computed(() => /^[A-Za-z0-9_]+$/.test(atcoderId.value))
</script>

<template>
  <div>
    <h2 class="fontstyle-ui-subtitle">自己紹介</h2>
  </div>
  <div class="self-stretch py-4 fontstyle-ui-body wrap-break-word whitespace-pre-wrap">
    {{ selfIntroduction }}
  </div>
  <hr class="w-full border-border-primary" />
  <div class="flex flex-col items-start gap-2 py-4 fontstyle-ui-body text-text-primary">
    <div v-if="githubId" class="flex items-center">
      <span>
        <img src="/src/assets/service_icons/github.svg" alt="GitHub" class="size-5" />
      </span>
      <span class="px-2">
        <TextLink v-if="isGithubIdValid" :href="`https://github.com/${githubId}`" new-tab
          >@{{ githubId }}</TextLink
        >
        <span v-else>@{{ githubId }}</span>
      </span>
    </div>
    <div v-if="xId" class="flex items-center">
      <span>
        <img src="/src/assets/service_icons/x.svg" alt="X" class="size-5" />
      </span>
      <span class="px-2">
        <TextLink v-if="isXIdValid" :href="`https://x.com/intent/user?screen_name=${xId}`" new-tab
          >@{{ xId }}</TextLink
        >
        <span v-else>@{{ xId }}</span>
      </span>
    </div>
    <div v-if="atcoderId" class="flex items-center gap-2">
      <span>AtCoder</span>
      <TextLink v-if="isAtcoderIdValid" :href="`https://atcoder.jp/users/${atcoderId}`" new-tab>{{
        atcoderId
      }}</TextLink>
      <span v-else>{{ atcoderId }}</span>
    </div>
  </div>
</template>
