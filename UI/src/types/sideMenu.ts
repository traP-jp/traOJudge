import type { Icon } from '@/types/icon'

export type SideMenuProps = {
  icon?: Icon
  text: string
  href?: string
  onClick?: () => void
}
