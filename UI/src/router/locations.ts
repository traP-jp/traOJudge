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
