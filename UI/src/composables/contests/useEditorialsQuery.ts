import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import { apiClient, type Contest, type Editorial, requireData } from '@/api/client'
import { ApiError } from '@/api/errors'
import { useContestPhase } from '@/composables/contests/useContestPhase'
import { useApiQuery } from '@/composables/useApiQuery'
import { useUserStore } from '@/stores/user'
import {
  canViewEditorial,
  getEarlyEditorialAccess,
  parseEditorialRoles
} from '@/utils/editorialAccess'

type Viewer = { viewerId: string; credentials: RequestCredentials; signal: AbortSignal }
const loadAccess = async (contestId: string, viewer: Viewer, hasFinished: boolean) => {
  if (!viewer.viewerId) return getEarlyEditorialAccess([], contestId)
  try {
    const { data } = await apiClient.GET('/me/roles', viewer)
    return getEarlyEditorialAccess(
      parseEditorialRoles(requireData(data)),
      contestId,
      viewer.viewerId
    )
  } catch (error) {
    viewer.signal.throwIfAborted()
    if (!hasFinished || (error instanceof ApiError && error.response.status === 401)) throw error
    // 終了後の公開解説は、ロール取得の失敗で閲覧できなくならないようにする。
    return getEarlyEditorialAccess([], contestId, viewer.viewerId)
  }
}

const listEditorials = async (
  problemId: string,
  viewer: Viewer,
  canView: (editorial: Editorial) => boolean
) => {
  const items: Editorial[] = []
  const seen = new Set<string>()
  let cursor: string | undefined
  do {
    const { data } = await apiClient.GET('/problems/{problemId}/editorials', {
      ...viewer,
      params: { path: { problemId }, query: { limit: 100, cursor } }
    })
    const page = requireData(data)
    items.push(
      ...page.items.filter((editorial) => editorial.problemId === problemId && canView(editorial))
    )
    const next: unknown = page.nextCursor
    if (next != null && typeof next !== 'string') throw new Error('Invalid editorial cursor')
    cursor = next ?? undefined
    if (cursor && seen.has(cursor)) throw new Error('Repeated editorial cursor')
    if (cursor) seen.add(cursor)
  } while (cursor)
  return items
}

export const useEditorialsQuery = (contest: MaybeRefOrGetter<Contest | undefined>) => {
  const session = useUserStore()
  const { phase } = useContestPhase(contest)
  const finished = computed(() => phase.value === 'finished')
  return useApiQuery({
    keyPrefix: 'contest',
    key: () => ['editorials', toValue(contest)?.id, finished.value],
    enabled: () => !!toValue(contest),
    fetch: async (signal) => {
      const value = toValue(contest)
      if (!value) throw new Error('Missing contest')
      const hasFinished = finished.value
      const viewer: Viewer = {
        viewerId: session.userId,
        credentials: session.isAuthenticated ? 'include' : 'omit',
        signal
      }
      if (!hasFinished && !viewer.viewerId) return { awaitingRelease: true, problems: [] }
      const access = await loadAccess(value.id, viewer, hasFinished)
      const { data } = await apiClient.GET('/contests/{contestId}/problems', {
        ...viewer,
        params: { path: { contestId: value.id } }
      })
      const problems = await Promise.all(
        [...requireData(data).items]
          .sort((a, b) => a.order - b.order || a.label.localeCompare(b.label))
          .map(async (problem) => {
            const [title, editorials] = await Promise.allSettled([
              apiClient
                .GET('/problems/{problemId}', {
                  ...viewer,
                  params: { path: { problemId: problem.problemId } }
                })
                .then(({ data }) => requireData(data).title),
              listEditorials(problem.problemId, viewer, (item) =>
                canViewEditorial(item, hasFinished, access)
              )
            ])
            for (const result of [title, editorials]) {
              if (
                result.status === 'rejected' &&
                result.reason instanceof ApiError &&
                result.reason.response.status === 401
              )
                throw result.reason
            }
            return {
              ...problem,
              title: title.status === 'fulfilled' ? title.value : null,
              editorials: editorials.status === 'fulfilled' ? editorials.value : [],
              error: editorials.status === 'rejected'
            }
          })
      )
      return {
        problems,
        awaitingRelease:
          !hasFinished &&
          !access.isOrganizer &&
          problems.every((p) => !p.error && p.editorials.length === 0)
      }
    }
  })
}

export const useEditorialQuery = (
  contest: MaybeRefOrGetter<Contest | undefined>,
  problemId: MaybeRefOrGetter<string>,
  editorialId: MaybeRefOrGetter<string>
) => {
  const session = useUserStore()
  const { phase } = useContestPhase(contest)
  const finished = computed(() => phase.value === 'finished')
  return useApiQuery({
    keyPrefix: 'contest',
    key: () => [
      'editorial',
      toValue(contest)?.id,
      toValue(problemId),
      toValue(editorialId),
      finished.value
    ],
    enabled: () => !!toValue(contest) && !!toValue(problemId) && !!toValue(editorialId),
    fetch: async (signal) => {
      const value = toValue(contest)
      if (!value) throw new Error('Missing contest')
      const id = toValue(editorialId),
        problem = toValue(problemId),
        hasFinished = finished.value
      const viewer: Viewer = {
        viewerId: session.userId,
        credentials: session.isAuthenticated ? 'include' : 'omit',
        signal
      }
      if (!hasFinished && !viewer.viewerId) return { awaitingRelease: true, editorial: undefined }
      const { data: problems } = await apiClient.GET('/contests/{contestId}/problems', {
        ...viewer,
        params: { path: { contestId: value.id } }
      })
      if (!requireData(problems).items.some((p) => p.problemId === problem))
        throw new ApiError(new Response(null, { status: 404 }))
      const access = await loadAccess(value.id, viewer, hasFinished)
      const { data } = await apiClient.GET('/editorials/{editorialId}', {
        ...viewer,
        params: { path: { editorialId: id } }
      })
      const editorial = requireData(data)
      if (editorial.id !== id || editorial.problemId !== problem)
        throw new ApiError(new Response(null, { status: 404 }))
      if (!canViewEditorial(editorial, hasFinished, access))
        throw new ApiError(new Response(null, { status: 403 }))
      return { awaitingRelease: false, editorial }
    }
  })
}
