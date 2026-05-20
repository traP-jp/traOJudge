import globals from 'globals'
import pluginJs from '@eslint/js'
import tseslint from 'typescript-eslint'
import pluginVue from 'eslint-plugin-vue'
import tailwind from 'eslint-plugin-tailwindcss'
import eslintPrettierConfig from 'eslint-config-prettier'
import unicorn from 'eslint-plugin-unicorn'
import eslintComments from '@eslint-community/eslint-plugin-eslint-comments'

export default [
  { files: ['**/*.{js,mjs,cjs,ts,vue}'] },
  {
    ignores: ['node_modules', 'dist', 'build', 'coverage', 'docs', 'public', 'src/api/generated/**']
  },
  { languageOptions: { globals: globals.browser } },
  pluginJs.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs['flat/recommended'],
  ...tailwind.configs['flat/recommended'],
  eslintPrettierConfig,
  { files: ['**/*.ts'], languageOptions: { parserOptions: { parser: tseslint.parser } } },
  { files: ['**/*.vue'], languageOptions: { parserOptions: { parser: tseslint.parser } } },
  {
    plugins: {
      unicorn,
      '@eslint-community/eslint-comments': eslintComments
    },
    rules: {
      'tailwindcss/classnames-order': 'error',
      // 任意値はデザインシステム外の値が紛れ込む可能性があるため警告する。
      'tailwindcss/no-arbitrary-value': 'warn',

      // CompositionAPI 形式が最新のVueで推奨されているため。
      'vue/component-api-style': ['error', ['script-setup']],

      // <script> を JS で書くと型の恩恵を失うため、全コンポーネントで TS を強制。
      'vue/block-lang': ['error', { script: { lang: 'ts' } }],

      // ランタイム宣言 ({ type: String, required: true } 形式) を使うと
      // 型情報を JS オブジェクトでも二重管理することになり、TS 型との乖離が起きる。
      // 型ベース宣言なら TS の型がそのまま Single Source of Truth になる。
      'vue/define-props-declaration': ['error', 'type-based'],
      'vue/define-emits-declaration': ['error', 'type-based'],

      // スタイルは Tailwind に集約してデザイントークンを一元管理する。
      'vue/no-restricted-block': ['error', 'style'],

      // any は型システムを部分的に無効化する可能性があるため禁止する。
      // 未知の型を扱いたいときは unknown を使い、絞り込みを強制する。
      '@typescript-eslint/no-explicit-any': 'error',

      // '../foo' 形式の親相対パスはファイル移動でリンクが切れるリスクがあるため禁止する。
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['../*'],
              message: "Use the '@/' alias instead of parent-relative imports."
            }
          ]
        }
      ],

      'vue/block-order': ['error', { order: ['script', 'template', 'style'] }],

      // eslint-disable は例外運用なので、なぜ外したかが将来の読み手に必要。
      '@eslint-community/eslint-comments/require-description': ['error', { ignore: [] }],

      // ifのネストは可読性に悪影響を与えるため禁止する。
      'no-restricted-syntax': [
        'error',
        {
          selector: 'IfStatement > BlockStatement IfStatement',
          message:
            'ネストした if は禁止です。早期 return / ガード節 / 関数分割で平坦化してください。'
        }
      ],

      // コールバックにはアロー関数を使う。function 式は独自の this を持つため、
      // Vue の Composition API や非同期処理で this を意図せずシャドウする事故が起きやすい。
      // アロー関数は this を字句的に継承するためこの種のバグを構造的に防げる。
      // また簡潔さも上がり、コールバックの本質 (引数→結果) が読み取りやすくなる。
      // auto-fix 可能なので PostToolUse フックで自動修正される。
      'prefer-arrow-callback': ['error', { allowNamedFunctions: false, allowUnboundThis: true }]
    }
  },
  {
    files: ['src/**/*.vue'],
    rules: {
      'unicorn/filename-case': ['error', { case: 'pascalCase' }]
    }
  },
  {
    files: ['src/**/*.{ts,tsx,js,mjs,cjs}'],
    rules: {
      'unicorn/filename-case': ['error', { case: 'camelCase' }]
    }
  }
]
