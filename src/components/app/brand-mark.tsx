/**
 * The PathKeep mark ("Summit P"): a mountain bowl and a winding road stem.
 * The mountain is graphite on light surfaces and near-white on dark ones,
 * so the file is picked by the resolved theme. Sources: docs/design/brand-icon.
 */
import { cn } from '@/lib/cn'
import { useTheme } from '@/lib/theme'
import darkMarkUrl from '@/assets/pathkeep-mark-dark.svg'
import lightMarkUrl from '@/assets/pathkeep-mark.svg'

export function BrandMark({
  className,
  label,
}: {
  className?: string
  label?: string
}) {
  const { resolved } = useTheme()
  return (
    <img
      src={resolved === 'dark' ? darkMarkUrl : lightMarkUrl}
      alt={label ?? ''}
      aria-hidden={label ? undefined : true}
      draggable={false}
      className={cn('size-8 select-none', className)}
    />
  )
}
