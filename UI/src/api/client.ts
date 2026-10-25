import createClient from 'openapi-fetch'

import { ApiError } from '@/api/errors'
import type { components, paths } from '@/api/generated/schema'

export type CurrentUser = components['schemas']['CurrentUser']
export type Contest = components['schemas']['Contest']
export type Editorial = components['schemas']['Editorial']
export type EditorialRole = Pick<components['schemas']['RoleAssignment'], 'role' | 'scope'>

export const apiClient = createClient<paths>({
  baseUrl: (import.meta.env.VITE_API_V1_BASE_URL || '/api/v1').replace(/\/$/, ''),
  credentials: 'include',
  fetch: (request) => fetch(request)
})
apiClient.use({
  onResponse({ response }) {
    if (!response.ok) throw new ApiError(response)
  }
})

export const requireData = <T>(data: T | undefined): T => {
  if (data === undefined) throw new Error('Empty API response')
  return data
}
