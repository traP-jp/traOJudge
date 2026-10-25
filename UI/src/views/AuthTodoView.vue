<script setup lang="ts">
import { RouterLink } from 'vue-router'
</script>

<template>
  <section class="mx-auto max-w-2xl px-6 py-12">
    <h1 class="mb-4 fontstyle-ui-subtitle text-text-primary">認証機能は準備中です</h1>
    <p class="mb-6 fontstyle-ui-body text-text-secondary">
      登録・外部サービスでのログイン・パスワード再設定は準備中です。
    </p>
    <RouterLink to="/login" class="fontstyle-ui-body text-text-primary underline">
      ログイン画面へ戻る
    </RouterLink>
  </section>
</template>
