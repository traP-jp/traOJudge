import { fileURLToPath } from 'node:url'
import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      setupFiles: ['./vitest.setup.ts'],
      // Monaco exposes a browser entry, but no default entry for Vitest's Node resolver.
      alias: [
        {
          find: /^monaco-editor$/,
          replacement: fileURLToPath(
            new URL('./node_modules/monaco-editor/esm/vs/editor/editor.api.js', import.meta.url)
          )
        }
      ],
      include: ['src/**/__tests__/*.test.ts']
    }
  })
)
