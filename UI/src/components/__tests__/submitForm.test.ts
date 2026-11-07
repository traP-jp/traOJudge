import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, type App } from 'vue'
import { webcrypto } from 'node:crypto'
import { Configuration, DefaultConfig } from '@/api/generated'
import SubmitForm from '@/components/SubmitForm.vue'

const { push } = vi.hoisted(() => ({ push: vi.fn() }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push }) }))
vi.mock('@/components/CodeBlock.vue', () => ({
  default: defineComponent({
    emits: ['update:modelValue'],
    setup(_, { emit }) {
      return () =>
        h('textarea', {
          id: 'source',
          onInput: (event: Event) =>
            emit('update:modelValue', (event.target as HTMLTextAreaElement).value)
        })
    }
  })
}))
let app: App | undefined
let host: HTMLDivElement | undefined
afterEach(() => {
  app?.unmount()
  host?.remove()
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})
const problemId = '00000000-0000-4000-8000-000000000010'
const languageId = '00000000-0000-4000-8000-000000000011'
const uploadId = '00000000-0000-4000-8000-000000000012'
const submissionId = '00000000-0000-4000-8000-000000000013'
const json = (value: unknown, status = 200) =>
  new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } })
async function mountForm(failUpload = false, filename = 'main.cpp') {
  DefaultConfig.config = new Configuration({ basePath: '/api/v1', credentials: 'include' })
  vi.stubGlobal('crypto', webcrypto)
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (url.endsWith('/languages'))
      return json({ items: [{ id: languageId, name: 'C++', versionLabel: '14.2', tags: [] }] })
    if (url.endsWith('/submissions/uploads'))
      return json(
        {
          uploadId,
          expiresAt: '2099-01-01T00:00:00Z',
          targets: [
            {
              fileId: languageId,
              path: 'main.cpp',
              method: 'PUT',
              url: 'https://storage.example/source',
              headers: { 'X-Checksum': 'required' },
              expiresAt: '2099-01-01T00:00:00Z'
            }
          ]
        },
        201
      )
    if (url === 'https://storage.example/source')
      return new Response('', { status: failUpload ? 500 : 200 })
    if (url.endsWith('/submissions') && init?.method === 'POST')
      return json(
        {
          submissionId,
          judgeRunId: languageId,
          problemVersionId: problemId,
          acceptedAt: '2026-11-07T00:00:00Z'
        },
        202
      )
    throw new Error(`Unexpected request: ${url}`)
  })
  vi.stubGlobal('fetch', fetchMock)
  host = document.createElement('div')
  document.body.append(host)
  app = createApp(SubmitForm, { problemId })
  app.mount(host)
  await vi.waitFor(() => expect(host?.querySelector('#source')).not.toBeNull())
  const source = host.querySelector<HTMLTextAreaElement>('#source')!
  source.value = 'あ'
  source.dispatchEvent(new Event('input', { bubbles: true }))
  const name = host.querySelector<HTMLInputElement>('#source-filename')!
  name.value = filename
  name.dispatchEvent(new Event('input', { bubbles: true }))
  await nextTick()
  host.querySelector<HTMLButtonElement>('button')!.click()
  return fetchMock
}

describe('SubmitForm', () => {
  it('uploads UTF-8 content before creating a submission with only uploadId', async () => {
    const fetchMock = await mountForm()
    await vi.waitFor(() =>
      expect(push).toHaveBeenCalledWith(`/problems/${problemId}/submissions/${submissionId}`)
    )
    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      '/api/v1/languages',
      '/api/v1/submissions/uploads',
      'https://storage.example/source',
      '/api/v1/submissions'
    ])
    const upload = fetchMock.mock.calls[1][1]!
    expect(JSON.parse(String(upload.body))).toEqual({
      problemId,
      languageId,
      context: { kind: 'practice' },
      files: [
        {
          path: 'main.cpp',
          byteLength: 3,
          sha256: 'dc5a4d3d82f7e15792959dc661538ae0e541ce66494516f5c9cfd9cd3308494d'
        }
      ]
    })
    expect(new Headers(upload.headers).get('Idempotency-Key')).toBeTruthy()
    const transfer = fetchMock.mock.calls[2][1]!
    expect(transfer.method).toBe('PUT')
    expect(transfer.credentials).toBe('omit')
    expect(transfer.headers).toEqual({ 'X-Checksum': 'required' })
    expect(Array.from(transfer.body as Uint8Array)).toEqual([227, 129, 130])
    expect(JSON.parse(String(fetchMock.mock.calls[3][1]!.body))).toEqual({ uploadId })
  })
  it('does not accept a submission when the file transfer fails', async () => {
    const fetchMock = await mountForm(true)
    await vi.waitFor(() =>
      expect(host?.querySelector('[role="alert"]')?.textContent).toContain('転送できません')
    )
    expect(fetchMock).toHaveBeenCalledTimes(3)
    expect(push).not.toHaveBeenCalled()
  })
  it('shows an invalid path before submitting and sends no upload request', async () => {
    const fetchMock = await mountForm(false, '../main.cpp')
    expect(host?.textContent).toContain('相対パスを指定してください')
    expect(host?.querySelector('#source-filename')?.getAttribute('aria-invalid')).toBe('true')
    expect(host?.querySelector<HTMLButtonElement>('button')?.disabled).toBe(true)
    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(push).not.toHaveBeenCalled()
  })
})
