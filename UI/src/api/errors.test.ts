import { expect, it } from 'vitest'

import { responseErrorMessage } from '@/api/errors'

it('uses a string error message from an API response', async () => {
  const response = Response.json({ message: '入力が不正です' }, { status: 400 })
  await expect(responseErrorMessage(response)).resolves.toBe('入力が不正です')
})

it.each(['null', '{"message":42}', '<html>Bad Gateway</html>', ''])(
  'preserves the HTTP error when the body has no usable message: %s',
  async (body) => {
    const response = new Response(body, { status: 502 })
    await expect(responseErrorMessage(response)).resolves.toBe('HTTP 502')
  }
)
