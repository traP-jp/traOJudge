import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'

import QueryState from '@/components/QueryState.vue'

it('does not render old content on a permission error', async () => {
  const wrapper = mount(QueryState, {
    props: { view: { status: 'success', data: 'secret', refreshing: false } },
    slots: { default: 'secret' }
  })
  await wrapper.setProps({ view: { status: 'error', message: '閲覧する権限がありません。' } })
  expect(wrapper.text()).not.toContain('secret')
  await wrapper.find('button').trigger('click')
  expect(wrapper.emitted('retry')).toHaveLength(1)
  wrapper.unmount()
})
