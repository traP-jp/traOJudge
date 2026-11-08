import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { createRouter, createWebHistory, type RouterHistory } from 'vue-router'

import { useCursorRoute } from '@/router/useCursorRoute'
import { cleanupQueries, mountQuery, setupSession } from '@/test/helpers'

let history: RouterHistory | undefined
afterEach(() => {
  cleanupQueries()
  history?.destroy()
})

const openList = async (url: string) => {
  window.history.replaceState({}, '', url)
  history = createWebHistory()
  const router = createRouter({
    history,
    routes: [{ path: '/:pathMatch(.*)*', component: { render: () => null } }]
  })
  await router.push(url)
  const session = setupSession()
  return { session, ...mountQuery(useCursorRoute, session, router) }
}

it('restores the previous cursor after remounting and browser history navigation', async () => {
  const { result, wrapper, router, session } = await openList(
    '/contest/a/submissions?scope=mine#results'
  )
  await result.changeCursor('second')
  await result.changeCursor('third')
  expect(result.previousCursor.value).toBe('second')
  wrapper.unmount()
  const restored = mountQuery(useCursorRoute, session, router).result
  await router.isReady()
  expect(restored.cursor.value).toBe('third')
  expect(restored.previousCursor.value).toBe('second')
  await restored.changeCursor(restored.previousCursor.value ?? undefined)
  expect(restored.cursor.value).toBe('second')
  expect(restored.previousCursor.value).toBeNull()
  const navigation = new Promise<void>((resolve) => {
    const remove = router.afterEach(() => {
      remove()
      resolve()
    })
  })
  router.back()
  await navigation
  expect(restored.cursor.value).toBe('third')
  expect(restored.previousCursor.value).toBe('second')
  await restored.changeCursor('second')
  await restored.changeCursor(undefined)
  expect(router.currentRoute.value.fullPath).toBe('/contest/a/submissions?scope=mine#results')
  expect(restored.previousCursor.value).toBeUndefined()
})

it('does not invent a previous cursor for direct links or reuse another list or user history', async () => {
  const { result, router, session } = await openList('/contest/a/submissions?cursor=direct')
  expect(result.cursor.value).toBe('direct')
  expect(result.previousCursor.value).toBeUndefined()
  await result.changeCursor(undefined)
  await result.changeCursor('second')
  expect(result.previousCursor.value).toBeNull()
  await router.push('/contest/a/submissions?scope=all&cursor=second')
  expect(result.previousCursor.value).toBeUndefined()
  await result.changeCursor('third')
  expect(result.previousCursor.value).toBe('second')
  session.session.reset()
  await flushPromises()
  expect(result.previousCursor.value).toBeUndefined()
})
