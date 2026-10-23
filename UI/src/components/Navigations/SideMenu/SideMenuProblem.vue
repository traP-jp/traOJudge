<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import SideMenuBase from '@/components/Navigations/SideMenu/SideMenuBase.vue'
import { problemLocation } from '@/router/locations'
import type { SideMenuProps } from '@/types/sideMenu'

const route = useRoute()
const router = useRouter()

const {
  problemId,
  isAdmin = false,
  isEditorialAvailable = false
} = defineProps<{
  problemId: string
  problemName: string
  isAdmin?: boolean
  isEditorialAvailable?: boolean
}>()

const mainContents = computed(() => {
  const contents: SideMenuProps[] = [
    {
      text: '問題',
      icon: 'assignment',
      href: router.resolve(problemLocation(problemId, 'home')).href
    },
    {
      text: '提出',
      icon: 'edit',
      href: router.resolve(problemLocation(problemId, 'submit')).href
    },
    {
      text: '提出一覧',
      icon: 'format_list_bulleted',
      href: router.resolve(problemLocation(problemId, 'submissions')).href
    }
  ]
  if (isEditorialAvailable) {
    contents.push({
      text: '解説',
      icon: 'assignment_turned_in',
      href: router.resolve(problemLocation(problemId, 'editorials')).href
    })
  }
  return contents
})

const bottomContents = computed((): SideMenuProps[] => {
  if (isAdmin) {
    return [
      {
        text: '問題設定',
        icon: 'edit_note',
        href: `/problems/${problemId}/edit`
      }
    ]
  } else {
    return []
  }
})

const currentTab = ref<number>(0)
watch(
  () => route.path,
  (path) => {
    let index = mainContents.value.findIndex((content) => content.href === path)
    if (index >= 0) {
      currentTab.value = index
      return
    }

    index = bottomContents.value.findIndex((content) => content.href === path)
    if (index >= 0) {
      const adjustedIndex = mainContents.value.length + index
      currentTab.value = adjustedIndex
      return
    }
  },
  { immediate: true }
)
</script>

<template>
  <SideMenuBase
    v-model="currentTab"
    :main-contents="mainContents"
    :bottom-contents="bottomContents"
  >
    <h1 class="fontstyle-ui-subtitle">{{ problemName }}</h1>
  </SideMenuBase>
</template>
