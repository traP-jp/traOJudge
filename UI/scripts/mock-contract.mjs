import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import Ajv from 'ajv/dist/2020.js'
import addFormats from 'ajv-formats'
import { loadOpenApi } from './openapi.mjs'

export const spec = loadOpenApi()
export const mock = JSON.parse(
  readFileSync(new URL('../src/api/mock/front-back-api-mock.json', import.meta.url), 'utf8')
)
const ajv = new Ajv({ strict: false, allErrors: true })
addFormats(ajv)
ajv.addSchema({ $id: 'contract', components: spec.components })
const validators = new Map()
export function validate(schema, value, label) {
  const key = JSON.stringify(schema)
  if (!validators.has(key))
    validators.set(key, ajv.compile({ ...schema, components: spec.components }))
  const validator = validators.get(key)
  assert.ok(
    validator(value),
    `${label}: ${ajv.errorsText(validator.errors)}; received ${JSON.stringify(value)}`
  )
}
export function findOperation(method, path) {
  const relative = path.replace(/^\/?api\/v1/, '').split('?')[0]
  for (const [template, item] of Object.entries(spec.paths)) {
    const match = relative.match(new RegExp(`^${template.replaceAll(/\{[^}]+\}/g, '([^/]+)')}$`))
    if (match && item[method])
      return { template, operation: item[method], params: item.parameters ?? [], match }
  }
  return null
}
export function dereference(schema) {
  if (Array.isArray(schema)) return schema.map(dereference)
  if (!schema || typeof schema !== 'object') return schema
  if (schema.$ref) return dereference(spec.components.schemas[schema.$ref.split('/').at(-1)])
  return Object.fromEntries(
    Object.entries(schema)
      .filter(([key]) => key !== 'example')
      .map(([key, value]) => [key, dereference(value)])
  )
}
export function isPreview(path) {
  return /^api\/v1\/contests\/[^/]+\/standings\/(details|team\/details|extended)$/.test(path)
}
