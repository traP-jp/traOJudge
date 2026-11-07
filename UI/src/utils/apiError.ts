import { ResponseError } from '@/api/generated'

export async function apiErrorMessage(cause: unknown): Promise<string> {
  if (!(cause instanceof ResponseError))
    return cause instanceof Error ? cause.message : '処理に失敗しました．'
  const payload: unknown = await cause.response
    .clone()
    .json()
    .catch(() => null)
  if (
    payload &&
    typeof payload === 'object' &&
    'message' in payload &&
    typeof payload.message === 'string'
  )
    return payload.message
  return `処理に失敗しました（${cause.response.status}）．`
}

export function requireEtag(response: Response): string {
  const etag = response.headers.get('ETag')
  if (!etag) throw new Error('更新に必要な情報を取得できませんでした．再読み込みしてください．')
  return etag
}
