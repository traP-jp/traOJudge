import { computed } from 'vue'

import { useUserStore } from '@/stores/user'

export const useMockAuthGuard = () => {
  const userStore = useUserStore()
  // shortcut: 仮ログインでの開発用。本認証の導入時に操作・対象ごとの権限判定へ置き換える。
  const canEditProblem = computed(() => import.meta.env.DEV && userStore.isAuthenticated)

  const requireMockLogin = async <T>(operation: () => Promise<T>): Promise<T> => {
    if (!canEditProblem.value) throw new Error('開発環境でのログインが必要です。')
    return operation()
  }

  return { canEditProblem, requireMockLogin }
}
