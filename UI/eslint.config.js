import pluginJs from '@eslint/js'
import eslintComments from '@eslint-community/eslint-plugin-eslint-comments'
import vitest from '@vitest/eslint-plugin'
import { vueTsConfigs, withVueTs } from '@vue/eslint-config-typescript'
import eslintPrettierConfig from 'eslint-config-prettier/flat'
import simpleImportSort from 'eslint-plugin-simple-import-sort'
import tailwind from 'eslint-plugin-tailwindcss'
import unicorn from 'eslint-plugin-unicorn'
import pluginVue from 'eslint-plugin-vue'
import accessibility from 'eslint-plugin-vuejs-accessibility'
import globals from 'globals'

const parentImportRestriction = {
  group: ['../*'],
  message: "親相対パスの代わりに '@/' エイリアスを使ってください。"
}

export default withVueTs(
  { rootDir: import.meta.dirname, allowComponentTypeUnsafety: false },
  {
    ignores: ['node_modules/**', 'dist/**', 'build/**', 'coverage/**', 'src/api/generated/**']
  },
  { files: ['**/*.{js,mjs,cjs,ts,tsx,vue}'] },
  pluginJs.configs.recommended,
  pluginVue.configs['flat/recommended'],
  vueTsConfigs.recommendedTypeChecked,
  {
    files: ['src/**/*.{js,mjs,cjs,ts,tsx,vue}'],
    languageOptions: { globals: globals.browser }
  },
  {
    files: ['*.{js,mjs,cjs}', 'scripts/**/*.mjs', 'vite.config.ts', 'vitest.config.ts'],
    languageOptions: { globals: globals.node }
  },
  ...accessibility.configs['flat/recommended'].map((config) => ({
    ...config,
    files: ['src/**/*.vue']
  })),
  {
    ...tailwind.configs.recommended,
    files: [...tailwind.configs.recommended.files, '**/*.mjs', '**/*.cjs']
  },
  {
    settings: { tailwindcss: { cssConfigPath: './src/assets/main.css' } },
    linterOptions: { reportUnusedDisableDirectives: 'error' },
    plugins: {
      unicorn,
      'simple-import-sort': simpleImportSort,
      '@eslint-community/eslint-comments': eslintComments
    },
    rules: {
      'simple-import-sort/imports': 'error',
      '@eslint-community/eslint-comments/require-description': 'error',
      'prefer-arrow-callback': ['error', { allowNamedFunctions: false, allowUnboundThis: true }],
      'no-restricted-syntax': [
        'error',
        {
          selector: 'IfStatement > BlockStatement IfStatement',
          message: 'ネストした if は禁止です。早期 return / ガード節で平坦化してください。'
        }
      ],
      // クラス順はPrettierで整える。
      'tailwindcss/classnames-order': 'off',
      // デザイントークン外の値を見落とさないよう警告する。
      'tailwindcss/no-arbitrary-value': 'warn'
    }
  },
  {
    files: ['**/*.{ts,tsx,vue}'],
    rules: {
      '@typescript-eslint/consistent-type-imports': 'error',
      '@typescript-eslint/switch-exhaustiveness-check': 'error'
    }
  },
  {
    files: ['src/**/*.{js,mjs,cjs,ts,tsx,vue}'],
    rules: {
      'no-restricted-imports': ['error', { patterns: [parentImportRestriction] }]
    }
  },
  {
    files: ['src/**/*.vue'],
    rules: {
      'no-unreachable': 'error',
      'getter-return': 'error',
      'vue/component-api-style': ['error', ['script-setup']],
      'vue/define-props-declaration': ['error', 'type-based'],
      'vue/define-emits-declaration': ['error', 'type-based'],
      'vue/no-restricted-block': ['error', 'style'],
      'vue/block-order': ['error', { order: ['script', 'template', 'style'] }],
      // 省略を undefined で表す prop もあるため、既定値は一律に要求しない。
      'vue/require-default-prop': 'off',
      'vue/no-required-prop-with-default': 'error',
      'vuejs-accessibility/label-has-for': ['error', { required: { some: ['nesting', 'id'] } }],
      'unicorn/filename-case': ['error', { case: 'pascalCase' }]
    }
  },
  {
    files: ['src/**/*.{js,mjs,cjs,ts,tsx}'],
    rules: { 'unicorn/filename-case': ['error', { case: 'camelCase' }] }
  },
  {
    ...vitest.configs.recommended,
    files: ['src/**/*.test.ts'],
    rules: {
      ...vitest.configs.recommended.rules,
      'vitest/no-disabled-tests': 'error',
      'vitest/valid-expect': ['error', { alwaysAwait: true }]
    }
  },
  {
    files: [
      'src/views/ProblemView.vue',
      'src/views/ProblemsView.vue',
      'src/views/UserView.vue',
      'src/views/problem/ProblemHome.vue',
      'src/views/problem/ProblemSubmissions.vue',
      'src/views/user/UserProblems.vue',
      'src/views/user/UserSubmissions.vue'
    ],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            parentImportRestriction,
            {
              group: ['@/api/generated', '@/api/generated/**'],
              allowTypeImports: true,
              message: '取得と認証の規則を共有するため、機能別composableを使ってください。'
            }
          ]
        }
      ]
    }
  },
  eslintPrettierConfig
)
