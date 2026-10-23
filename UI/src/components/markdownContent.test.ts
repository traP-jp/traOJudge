import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'

import MarkdownContent from '@/components/MarkdownContent.vue'

it('renders lists, escapes HTML, and rejects executable links', async () => {
  const wrapper = mount(MarkdownContent, {
    props: { source: '- one\n- two\n\n<script>alert(1)</script>\n\n[bad](javascript:alert(1))' }
  })
  expect(wrapper.findAll('li')).toHaveLength(2)
  expect(wrapper.find('script').exists()).toBe(false)
  expect(wrapper.find('a').exists()).toBe(false)
  await wrapper.setProps({ source: '"hello"', typographer: true })
  expect(wrapper.text()).toBe('“hello”')
  await wrapper.setProps({ typographer: false })
  expect(wrapper.text()).toBe('"hello"')
})
