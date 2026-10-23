import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'

import NumberedPagination from '@/components/controls/pagination/NumberedPagination.vue'

it('normalizes an out of range page to 1, never 0', () => {
  const wrapper = mount(NumberedPagination, { props: { page: 3, totalPages: 1 } })
  expect(wrapper.emitted('normalize')).toEqual([[1]])
  expect(wrapper.find('button').exists()).toBe(false)
  wrapper.unmount()
})
