import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useUserStore } from '@/stores/user'

export const useCursorRoute = () => {
  const route = useRoute()
  const router = useRouter()
  const session = useUserStore()
  const cursor = computed(() =>
    typeof route.query.cursor === 'string' && route.query.cursor ? route.query.cursor : undefined
  )
  const listKey = computed(() =>
    JSON.stringify([
      session.userId,
      router.resolve({ path: route.path, query: { ...route.query, cursor: undefined } }).fullPath
    ])
  )
  const trail = computed<(string | null)[]>(() => {
    // 再読み込み後も戻れるよう、取得位置は各履歴エントリーに保存する。
    const state = router.options.history.state
    const current = cursor.value
    return current &&
      state.cursorList === listKey.value &&
      state.cursorPage === current &&
      Array.isArray(state.cursorTrail) &&
      state.cursorTrail.every((value) => value === null || typeof value === 'string')
      ? state.cursorTrail
      : []
  })
  const previousCursor = computed(() => trail.value.at(-1))
  const changeCursor = (value: string | undefined) => {
    if (value === cursor.value) return Promise.resolve()
    const goingBack =
      previousCursor.value !== undefined && value === (previousCursor.value ?? undefined)
    return router.push({
      query: { ...route.query, cursor: value },
      hash: route.hash,
      state: {
        cursorList: listKey.value,
        cursorPage: value ?? null,
        cursorTrail: value
          ? goingBack
            ? trail.value.slice(0, -1)
            : [...trail.value, cursor.value ?? null]
          : []
      }
    })
  }
  return { cursor, previousCursor, changeCursor }
}
