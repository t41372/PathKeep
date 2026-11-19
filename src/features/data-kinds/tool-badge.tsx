/**
 * A tool's own icon and its name, the way every screen shows a tool.
 * The icon sits in a fixed 16 px box, so app icons and bare marks line up.
 */
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { toolLooks } from './tools'

function ToolIcon({ id }: { id: AgentToolId }) {
  const { icon, iconDark } = toolLooks[id]
  const img = 'size-4 shrink-0 object-contain'
  return (
    <span
      aria-hidden
      className="inline-flex size-4 shrink-0 items-center justify-center"
    >
      {iconDark ? (
        <>
          <img src={icon} alt="" className={cn(img, 'dark:hidden')} />
          <img src={iconDark} alt="" className={cn(img, 'hidden dark:block')} />
        </>
      ) : (
        <img src={icon} alt="" className={img} />
      )}
    </span>
  )
}

export function ToolBadge({
  id,
  className,
}: {
  id: AgentToolId
  className?: string
}) {
  const { t } = useI18n()
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 text-[13px] whitespace-nowrap',
        className,
      )}
    >
      <ToolIcon id={id} />
      {t(toolLooks[id].name)}
    </span>
  )
}
