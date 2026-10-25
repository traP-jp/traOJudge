import { computed, watch } from 'vue'
import { type LocationQuery, type LocationQueryRaw, useRoute, useRouter } from 'vue-router'

export const pageSize = 20
const defaults = { page: 1, difficultyBegin: 1, difficultyEnd: 10 }
export type ListConditions = typeof defaults

const integer = (value: unknown, fallback: number, min: number, max: number): number => {
  if (typeof value !== 'string' || !/^\d+$/.test(value)) return fallback
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed >= min && parsed <= max ? parsed : fallback
}

export const readListConditions = (query: LocationQuery): ListConditions => ({
  page: integer(query.page, defaults.page, 1, Math.floor(Number.MAX_SAFE_INTEGER / pageSize)),
  difficultyBegin: integer(query.difficultyBegin, defaults.difficultyBegin, 1, 10),
  difficultyEnd: integer(query.difficultyEnd, defaults.difficultyEnd, 1, 10)
})

const writeConditions = (query: LocationQuery, conditions: ListConditions): LocationQueryRaw => {
  const result: LocationQueryRaw = { ...query }
  for (const key of Object.keys(defaults) as (keyof ListConditions)[]) {
    result[key] = conditions[key] === defaults[key] ? undefined : String(conditions[key])
  }
  return result
}

// フィルタ変更とページのリセットを一度の遷移で反映する。
export const useListRoute = () => {
  const route = useRoute()
  const router = useRouter()
  const conditions = computed(() => readListConditions(route.query))
  const updateConditions = async (
    patch: Partial<ListConditions>,
    replace = false
  ): Promise<void> => {
    const next = { ...conditions.value, ...patch }
    if (patch.difficultyBegin !== undefined || patch.difficultyEnd !== undefined) next.page = 1
    const normalized = readListConditions(writeConditions({}, next) as LocationQuery)
    const target = {
      path: route.path,
      query: writeConditions(route.query, normalized),
      hash: route.hash
    }
    if (router.resolve(target).fullPath === route.fullPath) return
    if (replace) await router.replace(target)
    else await router.push(target)
  }
  watch(
    () => route.fullPath,
    () => {
      void updateConditions({}, true)
    },
    { immediate: true }
  )
  return { conditions, updateConditions }
}
