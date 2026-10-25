import { flushPromises, mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'

import { AuthenticationApi } from '@/api/generated'
import { testRouter } from '@/test/helpers'
import ResetPasswordMailSendView from '@/views/ResetPasswordMailSendView.vue'

it.each([
  { state: { email: 'user@example.com' }, email: 'user@example.com' },
  { state: {}, email: '' },
  { state: { email: 42 }, email: '' }
])('uses a string email from navigation state when resending: $state', async ({ state, email }) => {
  const router = testRouter()
  await router.push({ path: '/reset-password/mail-send', state })
  const send = vi.spyOn(AuthenticationApi.prototype, 'postRequestResetPassword').mockResolvedValue()
  const wrapper = mount(ResetPasswordMailSendView, { global: { plugins: [router] } })
  try {
    const resend = wrapper
      .findAll('button')
      .find((button) => button.text() === 'メールを再送信する')
    expect(resend).toBeDefined()
    await resend?.trigger('click')
    await flushPromises()
    expect(send).toHaveBeenCalledWith({ email: { email } })
  } finally {
    wrapper.unmount()
  }
})
