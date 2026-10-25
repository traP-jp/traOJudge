import { computed } from 'vue'
import { useRoute } from 'vue-router'

export const problemLocation = (
  id: string,
  tab: 'home' | 'submit' | 'submissions' | 'editorials' = 'home'
) => ({ name: `problem-${tab}`, params: { id } })
export const userLocation = (id: string) => ({ name: 'user-home', params: { id } })

export const useRouteId = () => {
  const route = useRoute()
  return computed(() => (typeof route.params.id === 'string' ? route.params.id : ''))
}

export const contestLocation = (contestId: string, tab: 'top' | 'editorials' = 'top') => ({
  name: `contest-${tab}`,
  params: { contestId }
})
export const contestEditorialLocation = (
  contestId: string,
  problemId: string,
  editorialId: string
) => ({
  name: 'contest-editorial',
  params: { contestId, problemId, editorialId }
})
export const useContestId = () => {
  const route = useRoute()
  return computed(() => (typeof route.params.contestId === 'string' ? route.params.contestId : ''))
}
