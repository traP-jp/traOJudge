import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { apiClient } from '@/api/client'
import { ApiError } from '@/api/errors'
import { queryClient, sessionQueryKey } from '@/api/queryClient'
import { deferred, me, setupSession } from '@/test/helpers'
import SettingsProfile from '@/views/settings/SettingsProfile.vue'

const profileResponse = (user = me, etag: string | null = '"profile-1"') => ({
  data: user,
  response: Response.json(user, { headers: etag ? { ETag: etag } : {} })
})
const wrappers: VueWrapper[] = []
const openProfile = async () => {
  const context = setupSession()
  // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- ESLintでは、.vueからimportしたコンポーネントの型を解決できないため。
  const wrapper: VueWrapper = mount(SettingsProfile, { global: { plugins: [context.pinia] } })
  wrappers.push(wrapper)
  await flushPromises()
  return { ...context, wrapper }
}
const saveButton = (wrapper: VueWrapper) =>
  wrapper.findAll('button').find((button) => button.text() === '保存')!
const reloadButton = (wrapper: VueWrapper) =>
  wrapper.findAll('button').find((button) => button.text().includes('最新の内容'))!

beforeEach(() => {
  vi.spyOn(apiClient, 'GET').mockResolvedValue(profileResponse())
})
afterEach(() => {
  wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
  queryClient.clear()
})

it('saves edits and cleared fields with the ETag without changing sessions', async () => {
  vi.spyOn(apiClient, 'GET').mockResolvedValue(
    profileResponse({ ...me, githubId: 'github-user', atcoderId: 'atcoder-user', xId: 'x-user' })
  )
  const updated = {
    ...me,
    displayName: '更新した名前',
    bio: '紹介',
    revision: '2'
  }
  const patch = vi
    .spyOn(apiClient, 'PATCH')
    .mockResolvedValue(profileResponse(updated, '"profile-2"'))
  const { wrapper, session } = await openProfile()
  const epoch = session.epoch
  const userKey = ['user', epoch, me.id, me.id]
  queryClient.setQueryData(userKey, me)
  await wrapper.get('#display-name').setValue(updated.displayName)
  await wrapper.get('#introduction').setValue(updated.bio)
  for (const service of ['GitHub', 'AtCoder', 'X']) {
    const input = wrapper.get<HTMLInputElement>(`input[aria-label="${service}ユーザー名"]`)
    expect(input.element.value).not.toBe('')
    await input.setValue('')
  }
  await saveButton(wrapper).trigger('click')
  await flushPromises()
  expect(patch).toHaveBeenCalledWith(
    '/me',
    expect.objectContaining({
      params: { header: { 'If-Match': '"profile-1"', Origin: window.location.origin } },
      body: {
        displayName: updated.displayName,
        bio: updated.bio,
        githubId: '',
        atcoderId: '',
        xId: ''
      }
    })
  )
  expect(session.user).toEqual(updated)
  expect(session.epoch).toBe(epoch)
  expect(queryClient.getQueryState(userKey)?.isInvalidated).toBe(true)
  expect(wrapper.text()).toContain('プロフィールが保存されました。')
  await saveButton(wrapper).trigger('click')
  expect(patch).toHaveBeenLastCalledWith(
    '/me',
    expect.objectContaining({
      params: { header: { 'If-Match': '"profile-2"', Origin: window.location.origin } }
    })
  )
})

it('keeps edits on conflict and requires an explicit reload before saving again', async () => {
  const patch = vi
    .spyOn(apiClient, 'PATCH')
    .mockRejectedValue(new ApiError(new Response(null, { status: 412 })))
  const { wrapper, session } = await openProfile()
  await wrapper.get('#display-name').setValue('保存したい名前')
  await saveButton(wrapper).trigger('click')
  await flushPromises()
  expect(wrapper.get<HTMLInputElement>('#display-name').element.value).toBe('保存したい名前')
  expect(wrapper.text()).toContain('ほかの操作でプロフィールが更新されました')
  expect(saveButton(wrapper).attributes('disabled')).toBeDefined()
  expect(session.user).toEqual(me)
  const current = { ...me, displayName: '別の画面で更新した名前', revision: '3' }
  vi.spyOn(apiClient, 'GET').mockResolvedValue(profileResponse(current, '"profile-3"'))
  await reloadButton(wrapper).trigger('click')
  await flushPromises()
  expect(wrapper.get<HTMLInputElement>('#display-name').element.value).toBe(current.displayName)
  expect(saveButton(wrapper).attributes('disabled')).toBeUndefined()
  patch.mockResolvedValue(profileResponse({ ...current, revision: '4' }, '"profile-4"'))
  await saveButton(wrapper).trigger('click')
  expect(patch).toHaveBeenLastCalledWith(
    '/me',
    expect.objectContaining({
      params: { header: { 'If-Match': '"profile-3"', Origin: window.location.origin } }
    })
  )
})

it('retains the draft on network failure and prevents duplicate in-flight saves', async () => {
  const pending = deferred<ReturnType<typeof profileResponse>>()
  const patch = vi.spyOn(apiClient, 'PATCH').mockReturnValue(pending.promise)
  const { wrapper } = await openProfile()
  await wrapper.get('#display-name').setValue('編集中')
  await saveButton(wrapper).trigger('click')
  await saveButton(wrapper).trigger('click')
  expect(patch).toHaveBeenCalledTimes(1)
  pending.reject(new Error('offline'))
  await flushPromises()
  expect(wrapper.get<HTMLInputElement>('#display-name').element.value).toBe('編集中')
  expect(saveButton(wrapper).attributes('disabled')).toBeUndefined()
  expect(wrapper.text()).toContain('プロフィールの保存に失敗しました')
})

it('ignores a late save after switching accounts and cancels the old request', async () => {
  const pending = deferred<ReturnType<typeof profileResponse>>()
  const patch = vi.spyOn(apiClient, 'PATCH').mockReturnValue(pending.promise)
  const { wrapper, session } = await openProfile()
  await wrapper.get('#display-name').setValue('古い利用者の編集')
  await saveButton(wrapper).trigger('click')
  const next = { ...me, id: 'user-b', displayName: 'Bob' }
  vi.spyOn(apiClient, 'GET').mockResolvedValue(profileResponse(next))
  await session.fetchCurrentUser()
  await flushPromises()
  pending.resolve(profileResponse({ ...me, displayName: '古い利用者の編集' }))
  await flushPromises()
  expect((patch.mock.calls[0][1] as RequestInit).signal?.aborted).toBe(true)
  expect(session.user).toEqual(next)
  expect(wrapper.get<HTMLInputElement>('#display-name').element.value).toBe('Bob')
  expect(wrapper.text()).not.toContain('プロフィールが保存されました。')
})

it.each(['load', 'save'])(
  'refreshes the session when %s detects a cookie-only account switch',
  async (operation) => {
    const next = { ...me, id: 'user-b', displayName: 'Bob' }
    const pending = deferred<ReturnType<typeof profileResponse>>()
    if (operation === 'load') vi.spyOn(apiClient, 'GET').mockReturnValueOnce(pending.promise)
    const { wrapper, session } = await openProfile()
    const epoch = session.epoch
    const privateKey = sessionQueryKey(epoch, ['problem', 'private-a'])
    queryClient.setQueryData(privateKey, { statement: 'Only A can read this' })
    vi.spyOn(apiClient, 'GET').mockResolvedValue(profileResponse(next))
    vi.spyOn(apiClient, 'PATCH').mockResolvedValue(profileResponse(next))
    if (operation === 'load') pending.resolve(profileResponse(next))
    if (operation === 'save') await saveButton(wrapper).trigger('click')
    await flushPromises()
    expect(session.user).toEqual(next)
    expect(session.epoch).toBeGreaterThan(epoch)
    expect(queryClient.getQueryData(privateKey)).toBeUndefined()
    expect(wrapper.get<HTMLInputElement>('#display-name').element.value).toBe('Bob')
    expect(wrapper.get('fieldset').attributes('disabled')).toBeUndefined()
    expect(saveButton(wrapper).attributes('disabled')).toBeUndefined()
    expect(wrapper.text()).not.toContain('プロフィールが保存されました。')
  }
)

it.each([401, 503])(
  'keeps old account data cleared when session refresh fails with %i',
  async (status) => {
    const pending = deferred<ReturnType<typeof profileResponse>>()
    vi.spyOn(apiClient, 'GET').mockReturnValueOnce(pending.promise)
    const { wrapper, session } = await openProfile()
    const privateKey = sessionQueryKey(session.epoch, ['problem', 'private-a'])
    queryClient.setQueryData(privateKey, { statement: 'Only A can read this' })
    vi.spyOn(apiClient, 'GET').mockRejectedValue(new ApiError(new Response(null, { status })))
    pending.resolve(profileResponse({ ...me, id: 'user-b' }))
    await flushPromises()
    expect(session.user).toBeNull()
    expect(session.isLoading).toBe(false)
    expect(queryClient.getQueryData(privateKey)).toBeUndefined()
    expect(wrapper.get<HTMLInputElement>('#display-name').element.value).toBe('')
    expect(saveButton(wrapper).attributes('disabled')).toBeDefined()
    expect(session.error !== null).toBe(status === 503)
  }
)

it('ignores a delayed profile after logout', async () => {
  const pending = deferred<ReturnType<typeof profileResponse>>()
  const get = vi.spyOn(apiClient, 'GET').mockReturnValue(pending.promise)
  const { wrapper, session } = await openProfile()
  session.reset()
  pending.resolve(profileResponse())
  await flushPromises()
  expect((get.mock.calls[0][1] as RequestInit).signal?.aborted).toBe(true)
  expect(wrapper.get<HTMLInputElement>('#display-name').element.value).toBe('')
  expect(session.user).toBeNull()
})

it('resets the session when saving returns 401', async () => {
  vi.spyOn(apiClient, 'PATCH').mockRejectedValue(new ApiError(new Response(null, { status: 401 })))
  const { wrapper, session } = await openProfile()
  await saveButton(wrapper).trigger('click')
  await flushPromises()
  expect(session.isAuthenticated).toBe(false)
  expect(wrapper.get<HTMLInputElement>('#display-name').element.value).toBe('')
})

it('disables saving without an ETag and allows retrying the initial load', async () => {
  vi.spyOn(apiClient, 'GET').mockResolvedValue(profileResponse(me, null))
  const { wrapper } = await openProfile()
  expect(saveButton(wrapper).attributes('disabled')).toBeDefined()
  vi.spyOn(apiClient, 'GET').mockResolvedValue(profileResponse())
  await reloadButton(wrapper).trigger('click')
  await flushPromises()
  expect(saveButton(wrapper).attributes('disabled')).toBeUndefined()
})

it('validates schema lengths as Unicode characters', async () => {
  const patch = vi.spyOn(apiClient, 'PATCH').mockResolvedValue(profileResponse())
  const { wrapper } = await openProfile()
  await wrapper.get('#display-name').setValue('a'.repeat(81))
  await wrapper.get('#introduction').setValue('あ'.repeat(4001))
  await saveButton(wrapper).trigger('click')
  expect(patch).not.toHaveBeenCalled()
  expect(wrapper.text()).toContain('1〜80文字')
  expect(wrapper.text()).toContain('4000文字以内')
  await wrapper.get('#display-name').setValue('😀'.repeat(80))
  await wrapper.get('#introduction').setValue('😀'.repeat(4000))
  await saveButton(wrapper).trigger('click')
  expect(patch).toHaveBeenCalledOnce()
})
