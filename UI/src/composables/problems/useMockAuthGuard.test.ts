import { createPinia, setActivePinia } from 'pinia'
import { afterEach, expect, it, vi } from 'vitest'

import { useMockAuthGuard } from '@/composables/problems/useMockAuthGuard'
import { useUserStore } from '@/stores/user'
import { me } from '@/test/helpers'

afterEach(() => vi.unstubAllEnvs())

it.each([true, false])('only permits signed-in development operations (DEV=%s)', async (dev) => {
  vi.stubEnv('DEV', dev)
  setActivePinia(createPinia())
  const session = useUserStore()
  const { canEditProblem, requireMockLogin } = useMockAuthGuard()
  const operation = vi.fn().mockResolvedValue('saved')

  expect(canEditProblem.value).toBe(false)
  await expect(requireMockLogin(operation)).rejects.toThrow()
  expect(operation).not.toHaveBeenCalled()

  session.$patch({ user: me, isInitialized: true })
  expect(canEditProblem.value).toBe(dev)
  const result = await requireMockLogin(operation).catch(() => 'denied')
  expect(result).toBe(dev ? 'saved' : 'denied')
  expect(operation).toHaveBeenCalledTimes(dev ? 1 : 0)
})
