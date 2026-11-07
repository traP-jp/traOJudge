import type { FileManifestEntry } from '@/api/generated'

export function sourceFilePathError(path: string): string {
  if (!path.trim()) return 'ファイル名を入力してください．'
  if ([...path].length > 1024) return 'ファイル名は1024文字以内で入力してください．'
  if (/\p{Cc}/u.test(path)) return 'ファイル名に制御文字は使えません．'
  const parts = path.split('/')
  if (
    path.includes('\\') ||
    /^[A-Za-z]:/.test(path) ||
    parts.some((part) => !part || part === '..' || part === '.')
  )
    return '相対パスを指定してください．絶対パス，空の要素，「.」「..」，バックスラッシュは使えません．'
  return ''
}

export async function sourceFileManifest(
  path: string,
  content: Uint8Array<ArrayBuffer>
): Promise<FileManifestEntry> {
  const error = sourceFilePathError(path)
  if (error) throw new Error(error)
  const digest = await crypto.subtle.digest('SHA-256', content)
  const sha256 = Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, '0')
  ).join('')
  return { path, byteLength: content.byteLength, sha256 }
}
