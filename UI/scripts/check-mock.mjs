import assert from 'node:assert/strict'
import { mock, spec, validate, findOperation, dereference, isPreview } from './mock-contract.mjs'

let checked = 0
let previews = 0
for (const route of mock.routes) {
  if (isPreview(route.endpoint)) {
    previews++
    continue
  }
  if (route.endpoint === 'mock-storage/:uploadId/:index' && route.method === 'put') continue
  const matched = findOperation(route.method, route.endpoint)
  assert.ok(matched, `Undefined route: ${route.method} ${route.endpoint}`)
  const { operation, params, template, match } = matched
  const names = [...template.matchAll(/\{([^}]+)\}/g)].map((item) => item[1])
  names.forEach((name, index) => {
    if (match[index + 1].startsWith(':')) return
    const param = params.find((p) => p.name === name)
    assert.ok(param, `Parameter ${name}`)
    validate(param.schema, match[index + 1], `${route.endpoint} ${name}`)
  })
  for (const response of route.responses) {
    const contract = operation.responses[response.statusCode]
    assert.ok(contract, `${operation.operationId}: undefined status ${response.statusCode}`)
    if (operation['x-authorization']?.status === 'pending') {
      assert.equal(response.statusCode, 403)
      assert.equal(JSON.parse(response.body).code, 'policy_not_decided')
    }
    for (const [name, header] of Object.entries(contract.headers ?? {})) {
      const value = response.headers.find((h) => h.key.toLowerCase() === name.toLowerCase())?.value
      assert.ok(value, `${operation.operationId}: missing ${name}`)
      if (!value.includes('{{')) validate(header.schema, value, `${operation.operationId}: ${name}`)
    }
    const schema = contract.content?.['application/json']?.schema
    if (!schema) {
      assert.equal(response.statusCode, 204)
      continue
    }
    if (!response.body.includes('{{'))
      validate(schema, JSON.parse(response.body), operation.operationId)
    checked++
  }
  const request = operation.requestBody?.content?.['application/json']?.schema
  if (!request || operation['x-authorization']?.status === 'pending') continue
  const bucket = mock.data.find((item) => item.name === operation.operationId + 'Request')
  assert.ok(bucket, `${operation.operationId}: missing request validation`)
  assert.deepEqual(
    JSON.parse(bucket.value),
    dereference(request),
    `${operation.operationId}: request schema drift`
  )
}
assert.equal(previews, 7, 'Keep the undefined standings previews')
for (const [name, schemaName] of Object.entries({
  problems: 'Problem',
  editorials: 'Editorial',
  submissions: 'Submission',
  'judge-runs': 'JudgeRun'
})) {
  const bucket = JSON.parse(mock.data.find((item) => item.name === name).value)
  for (const item of Object.values(bucket))
    validate(spec.components.schemas[schemaName], item, name)
}
const me = JSON.parse(mock.data.find((item) => item.name === 'me').value)
validate(spec.components.schemas.CurrentUser, me, 'me')
assert.deepEqual(me.roles, JSON.parse(mock.data.find((item) => item.name === 'roles').value).items)
console.log(`Mock contract: ${checked} responses checked; ${previews} undefined previews retained`)
