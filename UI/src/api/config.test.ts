import { afterEach, expect, it, vi } from 'vitest'

import { configureApi } from '@/api/config'
import { ProblemsApi } from '@/api/generated'
import { problem } from '@/test/helpers'

afterEach(() => {
  vi.unstubAllGlobals()
  configureApi()
})
it('applies runtime config even to an existing client and forwards cancellation', async () => {
  const client = new ProblemsApi()
  const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify(problem('a'))))
  vi.stubGlobal('fetch', fetch)
  configureApi('https://example.test/api/')
  const controller = new AbortController()
  await client.getProblem({ problemId: 'a' }, { signal: controller.signal })
  expect(fetch).toHaveBeenCalledWith(
    'https://example.test/api/problems/a',
    expect.objectContaining({ credentials: 'include', signal: controller.signal })
  )
})
