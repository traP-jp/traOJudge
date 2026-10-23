import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import type { Me, UserEmailAndPassword } from '@/api/generated'
import { AuthenticationApi, MeApi, ResponseError } from '@/api/generated'
import { queryClient } from '@/api/queryClient'

export const useUserStore = defineStore('user', () => {
  const user = ref<Me | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)
  const isInitialized = ref(false)
  const epoch = ref(0)
  let initPromise: Promise<void> | null = null
  let controller: AbortController | undefined

  const isAuthenticated = computed(() => !!user.value)
  const userId = computed(() => user.value?.id ?? '')
  const username = computed(() => user.value?.name ?? '')

  const beginSession = () => {
    controller?.abort()
    controller = new AbortController()
    user.value = null
    isInitialized.value = false
    isLoading.value = true
    error.value = null
    initPromise = null
    epoch.value++
    queryClient.clear()
    return { epoch: epoch.value, signal: controller.signal }
  }

  const assertCurrent = (operation: { epoch: number }): void => {
    if (operation.epoch !== epoch.value) throw new DOMException('Session changed', 'AbortError')
  }

  const readUser = async (operation: { epoch: number; signal: AbortSignal }): Promise<Me> => {
    const response = await new MeApi().getMe({ signal: operation.signal })
    assertCurrent(operation)
    user.value = response
    isInitialized.value = true
    return response
  }

  const recordFailure = (err: unknown, operation: { epoch: number }): void => {
    if (operation.epoch !== epoch.value) return
    const unauthorized = err instanceof ResponseError && err.response.status === 401
    isInitialized.value = unauthorized
    error.value = unauthorized
      ? null
      : 'ログイン状態を確認できませんでした。もう一度お試しください。'
  }

  const fetchCurrentUser = async (): Promise<Me> => {
    const operation = beginSession()
    try {
      return await readUser(operation)
    } catch (err) {
      recordFailure(err, operation)
      throw err
    } finally {
      if (operation.epoch === epoch.value) isLoading.value = false
    }
  }

  const initialize = (): Promise<void> => {
    if (initPromise) return initPromise
    if (isInitialized.value) return Promise.resolve()
    // localStorageに記録がなくてもCookieのセッションは有効な場合がある。
    const pending = fetchCurrentUser().then(
      () => undefined,
      () => undefined
    )
    initPromise = pending
    void pending.finally(() => {
      if (initPromise === pending) initPromise = null
    })
    return pending
  }

  const login = async (credentials: UserEmailAndPassword): Promise<void> => {
    const operation = beginSession()
    try {
      await new AuthenticationApi().postLogin(
        { userEmailAndPassword: credentials },
        { signal: operation.signal }
      )
      assertCurrent(operation)
      await readUser(operation)
    } catch (err) {
      recordFailure(err, operation)
      throw err
    } finally {
      if (operation.epoch === epoch.value) isLoading.value = false
    }
  }

  const logout = async (): Promise<void> => {
    const operation = beginSession()
    try {
      await new AuthenticationApi().postLogout({ signal: operation.signal })
      assertCurrent(operation)
      isInitialized.value = true
    } catch (err) {
      recordFailure(err, operation)
      if (!(err instanceof ResponseError && err.response.status === 401)) throw err
    } finally {
      if (operation.epoch === epoch.value) isLoading.value = false
    }
  }

  const reset = (): void => {
    beginSession()
    isInitialized.value = true
    isLoading.value = false
  }

  return {
    user,
    isLoading,
    error,
    isAuthenticated,
    isInitialized,
    epoch,
    userId,
    username,
    initialize,
    fetchCurrentUser,
    login,
    logout,
    reset
  }
})
