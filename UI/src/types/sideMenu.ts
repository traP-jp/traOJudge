import type { Icon } from '@/components/MaterialIcon.vue'

export type SideMenuProps = {
  icon?: Icon
  text: string
  href?: string
  onClick?: () => void
}
