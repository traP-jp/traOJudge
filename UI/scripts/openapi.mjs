import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import YAML from 'yaml'

export function loadOpenApi() {
  const rootFile = fileURLToPath(new URL('../../docs/API/front_back.yaml', import.meta.url))
  const documents = new Map()
  const read = (file) => {
    if (!documents.has(file)) documents.set(file, YAML.parse(readFileSync(file, 'utf8')))
    return documents.get(file)
  }
  const target = (ref, file) => {
    const [relative, pointer] = ref.split('#')
    const resolved = relative ? resolve(dirname(file), relative) : file
    const value = pointer
      .slice(1)
      .split('/')
      .reduce((node, key) => node[key.replaceAll('~1', '/').replaceAll('~0', '~')], read(resolved))
    return { file: resolved, value, key: `${resolved}#${pointer}` }
  }
  const root = read(rootFile)
  const schemaNames = new Map(
    Object.entries(root.components.schemas).map(([name, node]) => [
      target(node.$ref, rootFile).key,
      name
    ])
  )
  const visit = (node, file) => {
    if (Array.isArray(node)) return node.map((item) => visit(item, file))
    if (!node || typeof node !== 'object') return node
    if (node.$ref) {
      const resolved = target(node.$ref, file)
      const siblings = Object.fromEntries(Object.entries(node).filter(([key]) => key !== '$ref'))
      const name = schemaNames.get(resolved.key)
      return name
        ? { $ref: `#/components/schemas/${name}`, ...visit(siblings, file) }
        : { ...visit(resolved.value, resolved.file), ...visit(siblings, file) }
    }
    return Object.fromEntries(Object.entries(node).map(([key, value]) => [key, visit(value, file)]))
  }
  const bundled = visit(root, rootFile)
  for (const [name, node] of Object.entries(root.components.schemas)) {
    const resolved = target(node.$ref, rootFile)
    bundled.components.schemas[name] = visit(resolved.value, resolved.file)
  }
  return bundled
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  writeFileSync(process.argv[2], JSON.stringify(loadOpenApi(), null, 2))
}
