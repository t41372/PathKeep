/**
 * Small pieces every agent screen shares: harness badge, project chip,
 * KPI tile, coverage note, token breakdown bar, sparkline.
 */
import { CircleAlert, FolderGit2, Folder, FolderX, Shapes } from 'lucide-react'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { Skeleton } from '@/components/ui/skeleton'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { harnesses } from '~/data/archive'
import { big, num, pct } from '~/data/format'
import { totalTokens } from '~/data/fixture'
import type { HarnessId, Project, Tokens } from '~/data/types'

/** The harness's own icon in a fixed 16 px box, so PNG app icons and bare glyphs line up. */
export function HarnessIcon({ id, className }: { id: HarnessId; className?: string }) {
  const h = harnesses[id]
  const img = 'size-4 shrink-0 object-contain'
  return (
    <span aria-hidden className={cn('inline-flex size-4 shrink-0 items-center justify-center', className)}>
      {h.iconDark ? (
        <>
          <img src={h.icon} alt="" className={cn(img, 'dark:hidden')} />
          <img src={h.iconDark} alt="" className={cn(img, 'hidden dark:block')} />
        </>
      ) : (
        <img src={h.icon} alt="" className={img} />
      )}
    </span>
  )
}

export function HarnessBadge({
  id,
  withName,
  className,
}: {
  id: HarnessId
  withName?: boolean
  className?: string
}) {
  const h = harnesses[id]
  if (!withName)
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <span role="img" className={cn('inline-flex', className)} aria-label={h.name}>
            <HarnessIcon id={id} />
          </span>
        </TooltipTrigger>
        <TooltipContent>{h.name}</TooltipContent>
      </Tooltip>
    )
  return (
    <span className={cn('inline-flex items-center gap-1.5 text-[13px]', className)}>
      <HarnessIcon id={id} />
      {h.name}
    </span>
  )
}

export function ProjectIcon({ project, className }: { project: Project; className?: string }) {
  const Icon = project.kind === 'scratch' ? Shapes : project.gone ? FolderX : project.kind === 'repo' ? FolderGit2 : Folder
  return <Icon className={cn('size-3.5 shrink-0', project.gone ? 'text-muted-foreground/70' : 'text-muted-foreground', className)} aria-hidden />
}

export function ProjectChip({ project, link = true, className }: { project: Project; link?: boolean; className?: string }) {
  const body = (
    <>
      <ProjectIcon project={project} />
      <span className="truncate">{project.name}</span>
    </>
  )
  const cls = cn(
    'inline-flex max-w-[220px] min-w-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-[12px] text-muted-foreground',
    link && 'hover:bg-muted hover:text-foreground',
    className,
  )
  if (!link) return <span className={cls}>{body}</span>
  return (
    <Link to={`/agent/projects/${project.id}`} className={cls} onClick={(e) => e.stopPropagation()}>
      {body}
    </Link>
  )
}

/** The KPI tile Home and Insights use (same look as the app's StatCard). */
export function Stat({
  label,
  value,
  sub,
  to,
  loading,
  hint,
}: {
  label: ReactNode
  value: ReactNode
  sub?: ReactNode
  to?: string
  loading?: boolean
  hint?: ReactNode
}) {
  const inner = (
    <>
      <span className="flex items-center gap-1 text-[13px] text-muted-foreground">
        {label}
        {hint && (
          <Tooltip>
            <TooltipTrigger asChild>
              <span tabIndex={0} className="inline-flex text-muted-foreground/70 hover:text-foreground" aria-label="說明">
                <CircleAlert className="size-3.5" />
              </span>
            </TooltipTrigger>
            <TooltipContent className="max-w-[280px] leading-relaxed">{hint}</TooltipContent>
          </Tooltip>
        )}
      </span>
      {loading ? <Skeleton className="my-1 h-6 w-28" /> : <span className="text-2xl font-semibold tracking-[-0.02em] tabular">{value}</span>}
      {sub && <span className="text-xs text-muted-foreground">{sub}</span>}
    </>
  )
  const cls = 'flex min-w-0 flex-col gap-1.5 rounded-xl border bg-card px-[18px] py-4 shadow-card'
  return to ? (
    <Link to={to} className={cn(cls, 'transition-colors hover:bg-card/90')}>
      {inner}
    </Link>
  ) : (
    <div className={cls}>{inner}</div>
  )
}

/** "Known for N of M conversations": unknown is never shown as zero. */
export function Coverage({ known, total, what }: { known: number; total: number; what: string }) {
  if (known >= total) return null
  return (
    <span className="text-xs text-muted-foreground">
      {num(known)} / {num(total)} 段對話有{what}紀錄
    </span>
  )
}

export const tokenParts = [
  { key: 'output', label: '輸出', className: 'bg-brand' },
  { key: 'input', label: '輸入', className: 'bg-blue' },
  { key: 'cacheWrite', label: '寫入快取', className: 'bg-violet' },
  { key: 'cacheRead', label: '讀取快取', className: 'bg-foreground/25' },
] as const

/** One horizontal bar split by token kind, with a legend underneath. */
export function TokenBar({ tokens, legend = true, className }: { tokens: Tokens; legend?: boolean; className?: string }) {
  const total = totalTokens(tokens) || 1
  return (
    <div className={cn('flex flex-col gap-2', className)}>
      <div className="flex h-2 overflow-hidden rounded-full bg-muted">
        {tokenParts.map((part) => (
          <span key={part.key} className={part.className} style={{ width: `${(tokens[part.key] / total) * 100}%` }} />
        ))}
      </div>
      {legend && (
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {tokenParts.map((part) => (
            <span key={part.key} className="flex items-center gap-1.5">
              <span className={cn('size-2 rounded-[2px]', part.className)} />
              {part.label}
              <span className="text-foreground tabular">{big(tokens[part.key])}</span>
              <span className="tabular">{pct(tokens[part.key] / total, tokens[part.key] / total < 0.01 ? 1 : 0)}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  )
}

/** Weekly counts as small bars; plain divs, like the History detail panel. */
export function Sparkline({ values, className }: { values: number[]; className?: string }) {
  const max = Math.max(1, ...values)
  return (
    <span className={cn('flex h-5 items-end gap-px', className)} aria-hidden>
      {values.map((v, i) => (
        <span key={i} className={cn('w-[3px] rounded-[1px]', v ? 'bg-brand/70' : 'bg-muted')} style={{ height: `${v ? Math.max(12, (v / max) * 100) : 12}%` }} />
      ))}
    </span>
  )
}

export function Kv({ k, children }: { k: ReactNode; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[96px_minmax(0,1fr)] gap-3 py-1.5 text-[13px]">
      <dt className="text-muted-foreground">{k}</dt>
      <dd className="min-w-0 [overflow-wrap:anywhere]">{children}</dd>
    </div>
  )
}
