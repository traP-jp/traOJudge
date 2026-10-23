import { flushPromises } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'

import { readListConditions, useListRoute } from '@/router/listConditions'
import { cleanupQueries, mountQuery, setupSession, testRouter } from '@/test/helpers'

afterEach(cleanupQueries)

describe('list URL conditions', () => {
  it.each(['0', '-1', '2foo', '1.5', 'Infinity', '9007199254740991', ['2', '3'], null])(
    'normalizes invalid page %j',
    (page) => {
      expect(readListConditions({ page }).page).toBe(1)
    }
  )
  it('replaces invalid values, commits filters atomically, preserves hash and restores history', async () => {
    const router = testRouter()
    await router.push('/problems?page=-2&external=keep#anchor')
    const { result } = mountQuery(useListRoute, setupSession(), router)
    await flushPromises()
    expect(router.currentRoute.value.fullPath).toBe('/problems?external=keep#anchor')
    await result.updateConditions({ page: 3 })
    await result.updateConditions({ difficultyBegin: 2, difficultyEnd: 6 })
    expect(result.conditions.value).toEqual({ page: 1, difficultyBegin: 2, difficultyEnd: 6 })
    expect(router.currentRoute.value.query).toEqual({
      external: 'keep',
      difficultyBegin: '2',
      difficultyEnd: '6'
    })
    expect(router.currentRoute.value.hash).toBe('#anchor')
    router.back()
    await flushPromises()
    expect(result.conditions.value).toEqual({ page: 3, difficultyBegin: 1, difficultyEnd: 10 })
    router.forward()
    await flushPromises()
    expect(result.conditions.value.difficultyEnd).toBe(6)
  })
})
