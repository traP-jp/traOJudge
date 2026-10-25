import { AuthenticationApi } from '@/api/generated/auth/apis/AuthenticationApi'
import { MeApi } from '@/api/generated/auth/apis/MeApi'
import type { GetMe200Response } from '@/api/generated/auth/models/GetMe200Response'
import { Configuration, ResponseError } from '@/api/generated/auth/runtime'
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

const MOCK_SESSION_KEY = 'traOJudge_mockSession'
const apiConfig = new Configuration({ basePath: '/api/v1', credentials: 'include' })
const authApi = new AuthenticationApi(apiConfig)
const meApi = new MeApi(apiConfig)

export const useUserStore = defineStore('user', () => {
  const user = ref<GetMe200Response | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)
  const isInitialized = ref(false)
  let initPromise: Promise<void> | null = null

  const isAuthenticated = computed(() => user.value !== null)
  const userId = computed(() => user.value?.id ?? '')
  const username = computed(() => user.value?.displayName ?? '')

  const hasSessionFlag = (): boolean =>
    import.meta.env.DEV && sessionStorage.getItem(MOCK_SESSION_KEY) === 'true'

  const reset = () => {
    user.value = null
    error.value = null
    sessionStorage.removeItem(MOCK_SESSION_KEY)
  }

  const fetchCurrentUser = async (): Promise<GetMe200Response> => {
    isLoading.value = true
    error.value = null

    try {
      const profile = await meApi.getMe()
      user.value = profile
      return profile
    } catch (cause) {
      if (cause instanceof ResponseError && cause.response.status === 401) reset()
      error.value = cause instanceof Error ? cause.message : '利用者を取得できませんでした．'
      throw cause
    } finally {
      isLoading.value = false
    }
  }

  const initialize = async (): Promise<void> => {
    if (initPromise) return initPromise

    initPromise = (async () => {
      if (hasSessionFlag()) {
        try {
          await fetchCurrentUser()
        } catch {
          reset()
        }
      }
      isInitialized.value = true
    })()
    return initPromise
  }

  // TODO: 本物のメール・外部認証が実装されたら，モック専用トークンを使う処理を置き換える．
  const loginWithMock = async (): Promise<void> => {
    if (!import.meta.env.DEV) throw new Error('モックログインは開発環境でのみ利用できます．')
    isLoading.value = true
    error.value = null

    try {
      const result = await authApi.verifyEmailAuthentication({
        origin: window.location.origin,
        verifyEmailAuthenticationRequest: { token: 'mock-login' }
      })
      if (result.status !== 'signed-in') throw new Error('モックログインに失敗しました．')
      await fetchCurrentUser()
      sessionStorage.setItem(MOCK_SESSION_KEY, 'true')
    } catch (cause) {
      reset()
      error.value = cause instanceof Error ? cause.message : 'モックログインに失敗しました．'
      throw cause
    } finally {
      isLoading.value = false
    }
  }

  const logout = async (): Promise<void> => {
    isLoading.value = true
    try {
      if (import.meta.env.DEV) {
        await authApi.deleteSession({ origin: window.location.origin })
      }
    } catch {
      // モックサーバー停止後も，画面上の仮ログインは解除する．
    } finally {
      reset()
      isLoading.value = false
    }
  }

  return {
    user,
    isLoading,
    error,
    isAuthenticated,
    isInitialized,
    userId,
    username,
    initialize,
    hasSessionFlag,
    fetchCurrentUser,
    loginWithMock,
    logout,
    reset
  }
})
