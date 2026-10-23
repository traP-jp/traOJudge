import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'

import JudgeResultBadge from '@/components/submissions/JudgeResultBadge.vue'

it('updates the same badge from WJ to AC', async () => {
  const wrapper = mount(JudgeResultBadge, { props: { status: 'WJ' } })
  const oldClasses = wrapper.classes()
  await wrapper.setProps({ status: 'AC' })
  expect(wrapper.text()).toBe('AC')
  expect(wrapper.classes()).not.toEqual(oldClasses)
  wrapper.unmount()
})
