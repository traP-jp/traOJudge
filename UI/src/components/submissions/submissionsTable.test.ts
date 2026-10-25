import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'

import SubmissionsTable from '@/components/submissions/SubmissionsTable.vue'
import { submission } from '@/test/helpers'

it('renders both selected columns and supplied links without fetching', () => {
  const wrapper = mount(SubmissionsTable, {
    props: {
      rows: [{ ...submission('a'), problemHref: '/custom/problem', userHref: '/custom/user' }],
      columns: ['title', 'userName']
    }
  })
  expect(wrapper.findAll('a').map((link) => link.attributes('href'))).toEqual([
    '/custom/problem',
    '/custom/user'
  ])
  wrapper.unmount()
})
