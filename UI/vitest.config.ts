import { defineConfig, mergeConfig } from 'vitest/config'

import viteConfig from './vite.config.ts'

export default mergeConfig(
  viteConfig,
  defineConfig({
    // Monacoにはブラウザー用の入口しかないため、jsdomのモック用に明示する。
    resolve: {
      alias: [{ find: /^monaco-editor$/, replacement: 'monaco-editor/esm/vs/editor/editor.api.js' }]
    },
    test: { environment: 'jsdom', include: ['src/**/*.test.ts'], restoreMocks: true }
  })
)
