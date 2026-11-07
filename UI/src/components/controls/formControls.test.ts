import { mount, type VueWrapper } from '@vue/test-utils'
import { expect, it } from 'vitest'

import CopyableTextArea from '@/components/controls/CopyableTextArea.vue'
import PlainTextArea from '@/components/controls/PlainTextArea.vue'
import SelectBox from '@/components/controls/SelectBox.vue'
import PlainTextbox from '@/components/controls/textbox/PlainTextbox.vue'

it.each([
  [undefined, '0'],
  ['', '0'],
  ['日本語', '3'],
  ['😀あ', '2']
])('counts Unicode code points in the textarea: %s', (value, count) => {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- ESLintでは、.vueからimportしたコンポーネントの型を解決できないため。
  const wrapper: VueWrapper = mount(PlainTextArea, {
    props: { modelValue: value, displaysLength: true }
  })
  try {
    expect(wrapper.text()).toBe(count)
  } finally {
    wrapper.unmount()
  }
})

it('associates labels with unique controls, including generated and explicit IDs', () => {
  const wrapper = mount({
    // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- ESLintでは、.vueからimportしたコンポーネントの型を解決できないため。
    components: { PlainTextbox, PlainTextArea, CopyableTextArea, SelectBox },
    template: `<div>
      <PlainTextbox label="タイトル" />
      <PlainTextbox id="explicit-id" label="名前" />
      <PlainTextArea label="問題文" />
      <CopyableTextArea label="出力" model-value="output" />
      <SelectBox label="言語" :options="[{ id: 'cpp', name: 'C++' }]" />
    </div>`
  })
  try {
    const labels = wrapper.findAll('label')
    expect(labels).toHaveLength(5)
    for (const label of labels) {
      expect(label.element.control).not.toBeNull()
    }
    const ids = wrapper
      .findAll('input, textarea, select')
      .map((control) => control.attributes('id'))
    expect(new Set(ids).size).toBe(5)
    expect(ids).toContain('explicit-id')
    expect(ids).not.toContain('')
    expect(ids).not.toContain(undefined)
  } finally {
    wrapper.unmount()
  }
})
