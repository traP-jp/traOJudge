import assert from 'node:assert/strict'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { createRequire } from 'node:module'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.resolve('@mockoon/cli'))
const { MockoonServer } = require('@mockoon/commons-server')
const { parseDataFile } = require('./libs/data.js')

test('コンテストの提出と一覧は閲覧範囲を守り、再送を処理する', { timeout: 15000 }, async (t) => {
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
  const list = await fetch(`${base}/api/v1/me/submissions`, { headers: { Cookie: cookie } })
  const page = await list.json()
  assert.equal(page.items.length, 1)
  assert.equal(page.items[0].id, accepted.submissionId)
  assert.deepEqual(page.items[0].context, body.context)
  const run = await fetch(
    `${base}/api/v1/submissions/${accepted.submissionId}/judge-runs/${accepted.judgeRunId}`,
    { headers: { Cookie: cookie } }
  )
  const progress = await run.json()
  assert.equal(progress.submissionId, accepted.submissionId)
  assert.equal(typeof progress.revision, 'number')

  const read = (path, role = 'member') =>
    fetch(`${base}/api/v1${path}`, {
      headers: { Cookie: role ? `traojudge_mock_session=1; traojudge_mock_role=${role}` : '' }
    })
  assert.equal((await read('/languages', '')).status, 200)
  const contestPath = `/contests/${body.context.contestId}`
  assert.equal((await read(`${contestPath}/submissions`)).status, 403)
  assert.equal((await read(`${contestPath}/submissions`, '')).status, 403)
  const all = await read(`${contestPath}/submissions`, 'setter')
  assert.equal(all.status, 200, await all.clone().text())
  assert.equal((await all.json()).items[0].id, accepted.submissionId)
  assert.equal(
    (await read('/contests/7e75693c-91da-50e8-920c-7f268c7d99c4/submissions', '')).status,
    200
  )

  const teamContestId = '00f34a9a-2232-4ab3-a13e-187a6173475e'
  const teamUpload = await post(
    '/submissions/uploads',
    { ...body, context: { kind: 'contest', contestId: teamContestId } },
    crypto.randomUUID()
  )
  assert.equal(teamUpload.status, 201, await teamUpload.clone().text())
  const teamTarget = await teamUpload.json()
  await fetch(teamTarget.targets[0].url, { method: 'PUT', body: 'abc' })
  const teamCreated = await post(
    '/submissions',
    { uploadId: teamTarget.uploadId },
    crypto.randomUUID()
  )
  const teamAccepted = await teamCreated.json()
  assert.ok(teamAccepted.teamId)
  const teamList = await read(`/contests/${teamContestId}/teams/${teamAccepted.teamId}/submissions`)
  assert.equal(teamList.status, 200, await teamList.clone().text())
  assert.deepEqual(
    (await teamList.json()).items.map((item) => item.id),
    [teamAccepted.submissionId]
  )
  assert.equal((await read(`/contests/${teamContestId}/teams/not-mine/submissions`)).status, 403)
})
