import { computed, onScopeDispose, ref, shallowRef, watch, type WatchSource } from 'vue'
import { apiErrorMessage } from '@/utils/apiError'

export function useCursorPagination<T>(
  load: (
    cursor: string | undefined,
    signal: AbortSignal
  ) => Promise<{ items: T[]; nextCursor: string | null }>,
  key: WatchSource
) {
  const items = shallowRef<T[]>([])
  const cursors = ref<(string | undefined)[]>([undefined])
  const page = ref(0)
  const nextCursor = ref<string | null>(null)
  const isLoading = ref(false)
  const error = ref('')
  let controller: AbortController | undefined
  async function fetchPage(index: number) {
    controller?.abort()
    const request = new AbortController()
    controller = request
    isLoading.value = true
    error.value = ''
    try {
      const result = await load(cursors.value[index], request.signal)
      if (request.signal.aborted) return
      items.value = result.items
      nextCursor.value = result.nextCursor
      page.value = index
    } catch (cause) {
      if (request.signal.aborted) return
      const message = await apiErrorMessage(cause)
      if (!request.signal.aborted) error.value = message
    } finally {
      if (!request.signal.aborted) isLoading.value = false
    }
  }
  function next() {
    if (isLoading.value || !nextCursor.value) return
    cursors.value[page.value + 1] = nextCursor.value
    void fetchPage(page.value + 1)
  }
  function previous() {
    if (!isLoading.value && page.value > 0) void fetchPage(page.value - 1)
  }
  watch(
    key,
    () => {
      items.value = []
      cursors.value = [undefined]
      page.value = 0
      nextCursor.value = null
      void fetchPage(0)
    },
    { immediate: true }
  )
  onScopeDispose(() => controller?.abort())
  return {
    items,
    page: computed(() => page.value + 1),
    hasPrevious: computed(() => page.value > 0),
    hasNext: computed(() => nextCursor.value !== null),
    isLoading,
    error,
    next,
    previous,
    reload: () => fetchPage(page.value)
  }
}
