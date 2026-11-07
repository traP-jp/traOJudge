import type { FileManifestEntry } from '@/api/generated'

export async function sourceFileManifest(
  path: string,
  content: Uint8Array<ArrayBuffer>
): Promise<FileManifestEntry> {
  const parts = path.split('/')
  if (
    path.length > 1024 ||
    path.includes('\\') ||
    /^[A-Za-z]:/.test(path) ||
    parts.some((part) => !part || part === '..' || part === '.')
  ) {
    throw new Error('ファイル名には相対パスを指定してください．空の要素や「..」は使えません．')
  }
  const digest = await crypto.subtle.digest('SHA-256', content)
  const sha256 = Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, '0')
  ).join('')
  return { path, byteLength: content.byteLength, sha256 }
}
