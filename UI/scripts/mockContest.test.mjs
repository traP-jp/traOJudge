import assert from 'node:assert/strict'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { createRequire } from 'node:module'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.resolve('@mockoon/cli'))
const { MockoonServer } = require('@mockoon/commons-server')
const { parseDataFile } = require('./libs/data.js')

test('コンテストの提出は転送を確認し、再送を処理する', { timeout: 15000 }, async (t) => {
  const { environment } = await parseDataFile(
    fileURLToPath(new URL('../src/api/mock/front-back-api-mock.json', import.meta.url))
  )
  for (const route of environment.routes.filter((route) =>
    route.endpoint.endsWith('/clarifications/:clarificationId/answer')
  )) {
    for (const response of route.responses) response.latency = 40
  }
  const mock = new MockoonServer(environment, { enableAdminApi: false })
  mock.on('error', (code, error) => assert.fail(`${code}: ${error?.message ?? ''}`))
  const server = createServer(mock.createRequestListener())
  t.after(async () => {
    const closed = once(server, 'close')
    server.closeAllConnections()
    server.close()
    await closed
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const base = `http://127.0.0.1:${server.address().port}`
  const cookie = 'traojudge_mock_session=1'
  const post = (path, body, key, authenticated = true) =>
    fetch(`${base}/api/v1${path}`, {
      method: 'POST',
      headers: {
        Cookie: authenticated ? cookie : '',
        Origin: 'http://localhost:5173',
        'Content-Type': 'application/json',
        'Idempotency-Key': key
      },
      body: JSON.stringify(body)
    })
  const body = {
    problemId: 'afa35e34-c0cf-55ba-a3d4-16a59852d9bd',
    languageId: '97f77b41-292a-50dd-aadc-3cc7193fff6e',
    context: { kind: 'contest', contestId: '4bea2487-758f-5acc-b44b-2959fbdc887f' },
    files: [{ path: 'main.cpp', byteLength: 3, sha256: 'a'.repeat(64) }]
  }
  assert.equal((await post('/submissions/uploads', body, crypto.randomUUID(), false)).status, 401)
  const uploadKey = crypto.randomUUID()
  const uploaded = await post('/submissions/uploads', body, uploadKey)
  assert.equal(uploaded.status, 201, await uploaded.clone().text())
  const upload = await uploaded.json()
  assert.equal(upload.targets[0].path, 'main.cpp')
  const repeated = await (await post('/submissions/uploads', body, uploadKey)).json()
  assert.equal(repeated.uploadId, upload.uploadId)
  const submissionKey = crypto.randomUUID()
  const creation = { uploadId: upload.uploadId }
  assert.equal((await post('/submissions', creation, submissionKey)).status, 422)
  const stored = await fetch(upload.targets[0].url, {
    method: 'PUT',
    headers: upload.targets[0].headers,
    body: 'abc'
  })
  assert.equal(stored.status, 200, await stored.text())
  const created = await post('/submissions', creation, submissionKey)
  assert.equal(created.status, 202, await created.clone().text())
  const accepted = await created.json()
  const retry = await post('/submissions', creation, submissionKey)
  assert.deepEqual(await retry.json(), accepted)
})
