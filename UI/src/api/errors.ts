export const responseErrorMessage = async (response: Response): Promise<string> => {
  // エラー応答がJSONでなくても、元のHTTPエラーを表示できるようにする。
  const body: unknown = await response.json().catch(() => null)
  if (
    typeof body === 'object' &&
    body !== null &&
    'message' in body &&
    typeof body.message === 'string'
  ) {
    return body.message
  }
  return response.statusText || `HTTP ${response.status}`
}
