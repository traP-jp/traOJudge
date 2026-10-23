import type { Icon } from '@/components/icons/MaterialIcon.vue'

export type SideMenuProps = {
  icon?: Icon
  text: string
  href?: string
  onClick?: () => void
}
