import assert from 'node:assert/strict'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { createRequire } from 'node:module'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.resolve('@mockoon/cli'))
const { MockoonServer } = require('@mockoon/commons-server')
const { parseDataFile } = require('./libs/data.js')

test(
  'プロフィールの同時更新は1件だけ成功し、競合した更新ではデータを変更しない',
  { timeout: 10000 },
  async (t) => {
    const { environment } = await parseDataFile(
      fileURLToPath(new URL('../src/api/mock/front-back-api-mock.json', import.meta.url))
    )
    const route = environment.routes.find(
      (route) => route.method === 'patch' && route.endpoint === 'api/v1/me'
    )
    assert.ok(route)
    // 応答の選択から更新までを遅らせ、同じETagの要求が並行する条件を再現する。
    for (const response of route.responses) response.latency = 100
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
    const url = `http://127.0.0.1:${server.address().port}/api/v1/me`
    const cookie = 'traojudge_mock_session=1; traojudge_mock_role=setter'
    const read = () => fetch(url, { headers: { Cookie: cookie } })
    const patch = (etag, body, headers = {}) =>
      fetch(url, {
        method: 'PATCH',
        headers: {
          Cookie: cookie,
          Origin: 'http://localhost:5173',
          'Content-Type': 'application/json',
          'If-Match': etag,
          ...headers
        },
        body: JSON.stringify(body)
      })
    const initial = await read()
    assert.equal(initial.status, 200)
    const original = await initial.json()
    const etag = initial.headers.get('ETag')
    assert.ok(etag)
    const responses = await Promise.all([
      patch(etag, { displayName: '先の更新' }),
      patch(etag, { displayName: '後の更新' })
    ])
    assert.deepEqual(responses.map(({ status }) => status).sort(), [200, 412])
    const winner = responses.find(({ status }) => status === 200)
    const loser = responses.find(({ status }) => status === 412)
    const saved = await winner.json()
    assert.notEqual(saved.revision, original.revision)
    assert.deepEqual(saved.roles, original.roles)
    assert.equal((await loser.json()).code, 'precondition_failed')
    assert.equal(loser.headers.get('ETag'), winner.headers.get('ETag'))
    const current = await read()
    assert.deepEqual(await current.json(), saved)
    assert.equal(current.headers.get('ETag'), winner.headers.get('ETag'))

    const nextEtag = current.headers.get('ETag')
    for (const [body, headers, status] of [
      [{ bio: 'invalid' }, { Cookie: '' }, 401],
      [{ bio: 'invalid' }, { Origin: 'https://example.invalid' }, 403],
      [{ displayName: '' }, {}, 422]
    ]) {
      const rejected = await patch(nextEtag, body, headers)
      assert.equal(rejected.status, status)
      await rejected.arrayBuffer()
    }
    const stale = await patch(etag, { displayName: '古い更新' })
    assert.equal(stale.status, 412)
    await stale.arrayBuffer()
    assert.deepEqual(await (await read()).json(), saved)
    const updated = await patch(nextEtag, { bio: '引用符"と改行\n😀' })
    assert.equal(updated.status, 200)
    const profile = await updated.json()
    assert.equal(profile.bio, '引用符"と改行\n😀')
    assert.equal(profile.displayName, saved.displayName)
    assert.notEqual(profile.revision, saved.revision)
    assert.deepEqual(await (await read()).json(), profile)
  }
)
