import { describe, expect, it } from 'vitest'
import { webcrypto } from 'node:crypto'
import { sourceFileManifest, sourceFilePathError } from '@/utils/sourceFileManifest'

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
  it.each([
    '',
    '   ',
    '/main.cpp',
    '../main.cpp',
    'a/../b',
    'a//b',
    'C:\\main.cpp',
    'C:/main.cpp',
    'C:main.cpp',
    '//host/main.cpp',
    'a\\main.cpp',
    './main.cpp',
    'a/./b',
    'a/',
    'main\0.cpp',
    'main\t.cpp',
    'main\n.cpp',
    'main\u007f.cpp',
    'main\u0085.cpp',
    'a'.repeat(1025)
  ])('rejects invalid bundle path %s', async (path) => {
    await expect(sourceFileManifest(path, new Uint8Array())).rejects.toThrow()
  })
  it.each([
    'main.cpp',
    'src/main.cpp',
    '日本語.cpp',
    'with space.cpp',
    'a'.repeat(1024),
    '😀'.repeat(1024)
  ])('accepts relative paths within the schema character limit', (path) => {
    expect(sourceFilePathError(path)).toBe('')
  })
})
