import { describe, expect, it } from 'vitest'
import { webcrypto } from 'node:crypto'
import { sourceFileManifest } from '@/utils/sourceFileManifest'

describe('sourceFileManifest', () => {
  it('hashes UTF-8 bytes and counts bytes rather than characters', async () => {
    const original = globalThis.crypto
    Object.defineProperty(globalThis, 'crypto', { configurable: true, value: webcrypto })
    try {
      const manifest = await sourceFileManifest('src/main.cpp', new TextEncoder().encode('あ'))
      expect(manifest).toEqual({
        path: 'src/main.cpp',
        byteLength: 3,
        sha256: 'dc5a4d3d82f7e15792959dc661538ae0e541ce66494516f5c9cfd9cd3308494d'
      })
    } finally {
      Object.defineProperty(globalThis, 'crypto', { configurable: true, value: original })
    }
  })
  it.each(['', '/main.cpp', '../main.cpp', 'a/../b', 'a//b', 'C:\\main.cpp'])(
    'rejects invalid bundle path %s',
    async (path) => {
      await expect(sourceFileManifest(path, new Uint8Array())).rejects.toThrow()
    }
  )
})
