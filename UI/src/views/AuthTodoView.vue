<script setup lang="ts">
import { RouterLink } from 'vue-router'
</script>

<template>
  <section class="mx-auto max-w-2xl px-6 py-12">
    <h1 class="fontstyle-ui-subtitle mb-4 text-text-primary">認証機能は準備中です</h1>
    <p class="fontstyle-ui-body mb-6 text-text-secondary">
      新しい API に合わせて登録・外部認証・パスワード再設定を作り直しています．
    </p>
    <RouterLink to="/login" class="fontstyle-ui-body text-text-primary underline">
      ログイン画面へ戻る
    </RouterLink>
  </section>
</template>
