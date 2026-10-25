import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { after, test } from 'node:test'
import { fileURLToPath } from 'node:url'

import { ESLint } from 'eslint'
import { format, resolveConfig } from 'prettier'

const cwd = fileURLToPath(new URL('../', import.meta.url))
const directory = mkdtempSync(join(cwd, 'src/components/lint-'))
after(() => rmSync(directory, { recursive: true, force: true }))

const fixtures = {
  'UnreachableCode.vue': `<script setup lang="ts">
function value() { return 1; console.log('unreachable') }
const object = { get value() { if (Date.now() > 0) return 1 } }
</script><template><div>{{ value() }} {{ object.value }}</div></template>`,
  'unsafe.test.ts': `export const value = JSON.parse('{}')
value.run()
Promise.resolve(1)`,
  'ArbitraryValue.vue': '<template><div class="w-[317px]">Test</div></template>',
  'OptionalProp.vue': `<script setup lang="ts">
const { supportingText } = defineProps<{ supportingText?: string }>()
</script><template><p v-if="supportingText != null">{{ supportingText }}</p></template>`,
  'RequiredPropWithDefault.vue': `<script setup lang="ts">
const { label = '' } = defineProps<{ label: string }>()
</script><template><p>{{ label }}</p></template>`,
  'ValidComponent.vue': `<script setup lang="ts">
import { computed } from 'vue'
const { label } = defineProps<{ label: string }>()
const upper = computed(() => label.toUpperCase())
</script><template><p>{{ upper }}</p></template>`
}
for (const extension of ['js', 'mjs', 'cjs', 'tsx']) {
  fixtures[`BadName.${extension}`] =
    extension === 'cjs'
      ? "const value = require('../value.js'); console.log(value)"
      : "import value from '../value.js'; console.log(value)"
}
for (const [name, source] of Object.entries(fixtures)) {
  writeFileSync(join(directory, name), source)
}

const eslint = new ESLint({ cwd })
const messages = async (name) => {
  const [result] = await eslint.lintFiles([join(directory, name)])
  assert.equal(result.fatalErrorCount, 0, JSON.stringify(result.messages))
  return result.messages
}

test('Vue内の到達不能コードと値を返さないgetterを検出する', async () => {
  const rules = (await messages('UnreachableCode.vue')).map(({ ruleId }) => ruleId)
  assert.ok(rules.includes('no-unreachable'))
  assert.ok(rules.includes('getter-return'))
})

for (const extension of ['js', 'mjs', 'cjs', 'tsx']) {
  test(`${extension}でもファイル名と親相対importを検査する`, async () => {
    const rules = (await messages(`BadName.${extension}`)).map(({ ruleId }) => ruleId)
    assert.ok(rules.includes('unicorn/filename-case'))
    const config = await eslint.calculateConfigForFile(join(directory, `BadName.${extension}`))
    assert.equal(config.rules['no-restricted-imports'][0], 2)
    if (extension !== 'cjs') assert.ok(rules.includes('no-restricted-imports'))
  })
}

test('TSテストでも未検証のJSONと未処理Promiseを検出する', async () => {
  const rules = (await messages('unsafe.test.ts')).map(({ ruleId }) => ruleId)
  for (const rule of [
    'no-unsafe-assignment',
    'no-unsafe-call',
    'no-unsafe-member-access',
    'no-floating-promises'
  ]) {
    assert.ok(rules.includes(`@typescript-eslint/${rule}`), rule)
  }
})

test('任意値の警告でlintを失敗させる', async () => {
  const resultMessages = await messages('ArbitraryValue.vue')
  assert.equal(resultMessages.filter(({ severity }) => severity === 2).length, 0)
  const warning = resultMessages.find(({ ruleId }) => ruleId === 'tailwindcss/no-arbitrary-value')
  assert.equal(warning?.severity, 1)
  const result = spawnSync(
    process.execPath,
    [
      join(cwd, 'node_modules/eslint/bin/eslint.js'),
      join(directory, 'ArbitraryValue.vue'),
      '--max-warnings=0'
    ],
    { cwd, encoding: 'utf8' }
  )
  assert.ifError(result.error)
  assert.equal(result.status, 1, result.stderr)
  assert.match(result.stderr, /ESLint found too many warnings/)
})

test('通常のVue macroとcomputedは受け入れる', async () => {
  assert.deepEqual(await messages('ValidComponent.vue'), [])
})

test('省略可能なpropは既定値なしで受け入れる', async () => {
  assert.deepEqual(await messages('OptionalProp.vue'), [])
})

test('必須propへの既定値を検出する', async () => {
  const violation = (await messages('RequiredPropWithDefault.vue')).find(
    ({ ruleId }) => ruleId === 'vue/no-required-prop-with-default'
  )
  assert.equal(violation?.severity, 2)
})

test('PrettierがセミコロンなしとTailwindのクラス順を適用する', async () => {
  const config = await resolveConfig(join(cwd, 'src/main.ts'))
  assert.equal(
    await format('const value = 1;', { ...config, parser: 'typescript' }),
    'const value = 1\n'
  )
  const source = '<template><div class="items-center flex">Test</div></template>'
  const formatted = await format(source, { ...config, parser: 'vue' })
  assert.ok(formatted.includes('class="flex items-center"'))
  assert.equal(await format(formatted, { ...config, parser: 'vue' }), formatted)
})
