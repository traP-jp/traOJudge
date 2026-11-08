import assert from 'node:assert/strict'
import { createHash, randomUUID } from 'node:crypto'
import { setTimeout } from 'node:timers/promises'
import { mock, findOperation, validate, isPreview } from './mock-contract.mjs'

const base = process.env.MOCK_API_URL || 'http://localhost:4010'
const origin = 'http://localhost:5173'
let count = 0
async function request(method, path, body, headers = {}, status = 200) {
  const response = await fetch(`${base}/api/v1${path}`, {
    method: method.toUpperCase(),
    headers: {
      ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
      Origin: origin,
      'Idempotency-Key': randomUUID(),
      ...headers
    },
    body: body === undefined ? undefined : JSON.stringify(body)
  })
  const text = await response.text()
  assert.equal(response.status, status, `${method} ${path}: ${text}`)
  const operation = findOperation(method, '/api/v1' + path)?.operation
  assert.ok(operation, `${method} ${path}`)
  const contract = operation.responses[status]
  const value = text ? JSON.parse(text) : null
  const schema = contract.content?.['application/json']?.schema
  if (schema) validate(schema, value, `${method} ${path}`)
  for (const [name, header] of Object.entries(contract.headers ?? {})) {
    const actual = response.headers.get(name)
    assert.ok(actual, `${method} ${path}: missing ${name}`)
    validate(header.schema, actual, name)
  }
  count++
  return { value, etag: response.headers.get('ETag'), response }
}

for (const route of mock.routes.filter((r) => r.method === 'get' && !r.endpoint.includes(':'))) {
  if (isPreview(route.endpoint)) {
    const response = await fetch(`${base}/${route.endpoint}`)
    assert.equal(response.status, 200)
    await response.json()
    continue
  }
  await request(
    'get',
    '/' + route.endpoint.replace('api/v1/', ''),
    undefined,
    {},
    route.responses.at(-1).statusCode
  )
}
const first = await request('get', '/problems?limit=1')
assert.equal(first.value.items.length, 1)
assert.ok(first.value.nextCursor)
const second = await request('get', `/problems?limit=1&cursor=${first.value.nextCursor}`)
assert.notEqual(second.value.items[0].id, first.value.items[0].id)
const problemId = first.value.items[0].id
const ownId = (await request('get', '/me')).value.id
const contestId = '260b98b4-28de-5c53-8ebc-0f4980b3a0c0'
const teamContestId = '62bdcafc-4ece-5a05-9a41-b4625c183d12'
const runningContestId = '3c381dc5-8095-5bb4-9257-bd1e25588c5c'
const seeded = Object.values(
  JSON.parse(mock.data.find((item) => item.name === 'submissions').value)
)
const all = (await request('get', `/contests/${contestId}/submissions?limit=20`)).value
assert.equal(all.items.length, 20)
assert.ok(all.nextCursor)
assert.ok(all.items.every((item) => item.context.contestId === contestId))
const continuation = (
  await request('get', `/contests/${contestId}/submissions?limit=20&cursor=${all.nextCursor}`)
).value
assert.ok(
  continuation.items.every((item) => !all.items.some((previous) => previous.id === item.id))
)
const mine = (await request('get', '/me/submissions')).value
assert.ok(mine.items.length)
assert.ok(mine.items.every((item) => item.userId === ownId))
const privateSubmission = seeded.find(
  (item) =>
    item.context.contestId === contestId &&
    item.problemId === 'a851bcf7-36f4-5641-8353-01eb71ae382f' &&
    item.userId !== ownId
)
assert.ok(privateSubmission)
await request('get', `/problems/${privateSubmission.problemId}`, undefined, {}, 403)
await request('get', `/submissions/${privateSubmission.id}`)
const privateRun = (
  await request(
    'get',
    `/submissions/${privateSubmission.id}/judge-runs/${privateSubmission.currentJudgeRunId}`
  )
).value
assert.equal(privateRun.state, 'completed')
assert.equal(privateRun.result.verdict, 'AC')
const wrong = all.items.find((item) => item.userId !== ownId)
const wrongRun = (
  await request('get', `/submissions/${wrong.id}/judge-runs/${wrong.currentJudgeRunId}`)
).value
assert.equal(wrongRun.result.verdict, 'WA')
await request(
  'get',
  `/submissions/${wrong.id}/judge-runs/${privateSubmission.currentJudgeRunId}`,
  undefined,
  {},
  404
)
await request('get', `/contests/${runningContestId}/submissions`, undefined, {}, 403)
const hidden = seeded.find(
  (item) => item.context.contestId === runningContestId && item.userId !== ownId
)
await request('get', `/submissions/${hidden.id}`, undefined, {}, 403)
await request(
  'get',
  `/submissions/${hidden.id}/judge-runs/${hidden.currentJudgeRunId}`,
  undefined,
  {},
  403
)
const teamId = '2cfda6b0-5f87-50b7-aeca-0b8fb2aeee49'
const teamList = (await request('get', `/contests/${teamContestId}/teams/${teamId}/submissions`))
  .value
assert.ok(teamList.items.length)
assert.ok(
  teamList.items.every((item) => item.teamId === teamId && item.context.contestId === teamContestId)
)
await request(
  'get',
  `/contests/${teamContestId}/teams/14bff32c-8a1d-5a21-b297-eff3a8f609bc/submissions`,
  undefined,
  {},
  403
)
await request('get', `/contests/${teamContestId}/submissions`)
const search = await request('get', '/problems?query=' + encodeURIComponent('二数'))
assert.ok(search.value.items.length)
assert.ok(search.value.items.every((p) => p.title.includes('二数')))
assert.equal(
  (await request('get', `/problems?createdByUserId=${randomUUID()}`)).value.items.length,
  0
)
await request('get', '/problems?query=' + encodeURIComponent('`"\\\''))
await request('post', '/problems', { title: 'old', difficulty: 1 }, {}, 400)
await request('post', '/problems', { title: '' }, {}, 400)
await request('post', '/problems', { title: 'Origin check' }, { Origin: 'null' }, 403)
const title = `契約検証 ${randomUUID()} "quoted"`
const created = await request('post', '/problems', { title, statement: 'first\nsecond' }, {}, 201)
assert.equal(created.value.title, title)
assert.equal(created.value.visibility, 'private')
const path = `/problems/${created.value.id}`
assert.equal((await request('get', path)).value.statement, 'first\nsecond')
await request('patch', path, { title: 'missing etag' }, {}, 428)
await request('patch', path, { title: 'stale' }, { 'If-Match': '"stale"' }, 412)
const updated = await request(
  'patch',
  path,
  { title: title + ' updated' },
  { 'If-Match': created.etag }
)
assert.notEqual(updated.etag, created.etag)
const saved = await request('get', path)
assert.equal(saved.value.title, title + ' updated')
assert.equal(saved.etag, updated.etag)
await request('patch', path, { statement: 'stale' }, { 'If-Match': created.etag }, 412)
await request('put', path + '/publication', undefined, {}, 403)
await request('delete', path + '/publication', undefined, {}, 204)
assert.equal((await request('get', path)).value.visibility, 'private')
const editorial = await request(
  'post',
  '/editorials',
  { problemId, title: '解説', body: 'line1\n"line2"' },
  {},
  201
)
const editorialPath = '/editorials/' + editorial.value.id
const edited = await request(
  'patch',
  editorialPath,
  { body: '保存後の本文' },
  { 'If-Match': editorial.etag }
)
assert.equal((await request('get', editorialPath)).value.body, '保存後の本文')
assert.equal((await request('get', editorialPath)).etag, edited.etag)
const editorials = (await request('get', `/problems/${problemId}/editorials`)).value.items
assert.ok(editorials.some((item) => item.id === editorial.value.id))
for (const method of ['delete', 'put'])
  await request(
    method,
    editorialPath + (method === 'put' ? '/publication' : ''),
    undefined,
    {},
    403
  )
await request('delete', editorialPath + '/publication', undefined, {}, 403)
const cases = await request('get', `/problems/${problemId}/testcases`)
await request('get', `/problems/${problemId}/testcases/${cases.value.items[0].id}`)
await request('post', `/problems/${problemId}/testcases`, {}, {}, 403)
for (const method of ['put', 'delete'])
  await request(method, `/problems/${problemId}/testcases/${cases.value.items[0].id}`, {}, {}, 403)
const me = await request('get', '/me')
const changedMe = await request(
  'patch',
  '/me',
  { bio: 'profile\n"text"', atcoderId: 'sample' },
  { 'If-Match': me.etag }
)
assert.notEqual(me.etag, changedMe.etag)
assert.equal((await request('get', '/users/' + ownId)).value.bio, 'profile\n"text"')
await request('get', '/users/' + randomUUID())
await request('post', '/auth/email/verifications', { token: 'mock-login' })
const languageId = (await request('get', '/languages')).value.items[0].id
const content = 'int main() {}\n'
const file = {
  path: 'main.cpp',
  byteLength: Buffer.byteLength(content),
  sha256: createHash('sha256').update(content).digest('hex')
}
const upload = (
  await request(
    'post',
    '/submissions/uploads',
    { problemId, languageId, context: { kind: 'practice' }, files: [file] },
    {},
    201
  )
).value
assert.equal(upload.targets[0].path, file.path)
const transfer = await fetch(upload.targets[0].url, {
  method: upload.targets[0].method,
  headers: upload.targets[0].headers,
  body: content
})
assert.ok(transfer.ok)
const accepted = await request('post', '/submissions', { uploadId: upload.uploadId }, {}, 202)
assert.equal(
  accepted.response.headers.get('Location'),
  `/api/v1/submissions/${accepted.value.submissionId}`
)
const submission = (await request('get', '/submissions/' + accepted.value.submissionId)).value
assert.equal(submission.problemId, problemId)
assert.equal(submission.languageId, languageId)
assert.deepEqual(submission.context, { kind: 'practice' })
const runPath = `/submissions/${submission.id}/judge-runs/${submission.currentJudgeRunId}`
let run = (await request('get', runPath)).value
assert.equal(run.state, 'queued')
assert.equal(run.totalCaseCount, null)
const states = new Set([run.state])
const counts = new Set([run.completedCaseCount])
const deadline = Date.now() + 20000
while (run.state !== 'completed' && Date.now() < deadline) {
  await setTimeout(500)
  const next = (await request('get', runPath)).value
  assert.ok(next.revision >= run.revision)
  assert.ok(next.completedCaseCount >= run.completedCaseCount)
  if (next.revision === run.revision) assert.deepEqual(next, run)
  run = next
  states.add(run.state)
  counts.add(run.completedCaseCount)
}
assert.deepEqual([...states], ['queued', 'preparing', 'running', 'aggregating', 'completed'])
assert.deepEqual([...counts], [0, 1, 2, 3])
assert.equal(run.result.verdict, 'AC')
assert.equal(run.completedCaseCount, run.totalCaseCount)
assert.deepEqual((await request('get', runPath)).value, run)
assert.ok(
  (await request('get', '/me/submissions')).value.items.some((item) => item.id === submission.id)
)
await request('delete', path, undefined, {}, 204)
await request('get', path, undefined, {}, 404)
await request('delete', '/auth/session', undefined, {}, 204)
console.log(`Mock HTTP contract: ${count} requests passed`)
