import { useUserStore } from '@/stores/user'

// TODO: バックエンドの認可が実装されたら，操作ごとの役割と対象範囲を確認する．
export const useMockAuthGuard = () => {
  const userStore = useUserStore()

  const requireMockLogin = async <T>(operation: () => Promise<T>): Promise<T> => {
    if (!userStore.isAuthenticated) throw new Error('ログインが必要です．')
    return operation()
  }

  return { requireMockLogin }
}
