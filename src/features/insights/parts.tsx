/**
 * Building blocks shared by the Insights overview and its drill-ins: KPI
 * tiles, the per-card status (stale / limited / failed) and the drill-in
 * header.
 *
 * Responsible for: how a section's state is shown, consistently.
 * Not responsible for: fetching or deciding what a card contains.
 */
import type { UseQueryResult } from '@tanstack/react-query'
import { ArrowLeft, Info } from 'lucide-react'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { CardError } from '@/components/app/section-card'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Skeleton } from '@/components/ui/skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Spinner } from '@/components/ui/spinner'
import type { Section } from '@/lib/backend-client/insights'
import { cn } from '@/lib/cn'
import type {
  CoreIntelligenceSectionMeta,
  KpiMetric,
} from '@/lib/core-intelligence/types'
import { useFormat, useI18n } from '@/lib/i18n'
import { isRangeId, rangeDays, type RangeId } from './range'

const ranges = Object.keys(rangeDays) as RangeId[]

export function ListSkeleton({
  rows,
  height = 32,
}: {
  rows: number
  height?: number
}) {
  return (
    <div className="flex flex-col gap-2" aria-hidden>
      {Array.from({ length: rows }, (_, index) => (
        <Skeleton key={index} style={{ height }} />
      ))}
    </div>
  )
}

export function KpiTile({
  label,
  value,
  metric,
  hint,
  loading,
  detail,
}: {
  label: string
  value: ReactNode
  /** Adds the change against the previous period when the backend has one. */
  metric?: KpiMetric
  /** What the change is compared against, shown on hover. */
  hint?: string
  loading: boolean
  /** A short line under the value. */
  detail?: ReactNode
}) {
  const format = useFormat()
  const change = metric?.changePercent
  return (
    <div
      role="group"
      aria-label={label}
      aria-busy={loading}
      className="flex min-w-0 flex-col gap-1 rounded-xl border bg-card px-4 py-3.5 shadow-card"
    >
      <span className="text-[13px] text-muted-foreground">{label}</span>
      {loading ? (
        <Skeleton className="my-1 h-6 w-24" />
      ) : (
        <span className="flex items-baseline gap-2">
          <span className="truncate text-[22px] font-semibold tabular">
            {value}
          </span>
          {change != null && Number.isFinite(change) && (
            <span
              title={hint}
              className={cn(
                'text-xs tabular',
                change >= 0 ? 'text-green' : 'text-muted-foreground',
              )}
            >
              {format.percent(change / 100, true)}
            </span>
          )}
        </span>
      )}
      {detail && !loading && (
        <span className="truncate text-xs text-muted-foreground">{detail}</span>
      )}
    </div>
  )
}

/**
 * Freshness and evidence for one section, for the card's header. Shows a
 * spinner while the background job after a backup is still refreshing it,
 * and an info button that says when it was computed, from what, and any
 * reason the backend gave for a limited result.
 */
export function SectionStatus({
  meta,
  explain,
}: {
  meta: CoreIntelligenceSectionMeta | null | undefined
  /** How the number is counted, in the user's words. */
  explain?: ReactNode
}) {
  const { t } = useI18n()
  const format = useFormat()
  if (!meta && !explain) return null
  const stale = meta?.state === 'stale'
  const limited = meta?.state === 'degraded' || meta?.state === 'disabled'
  return (
    <span className="flex shrink-0 items-center gap-1.5">
      {stale && (
        <span className="flex items-center gap-1 text-xs text-muted-foreground">
          <Spinner className="size-3" />
          {t('insights.status.updating')}
        </span>
      )}
      {limited && (
        <span className="rounded-md bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">
          {t('insights.status.limited')}
        </span>
      )}
      <Popover>
        <PopoverTrigger
          aria-label={t('insights.status.about')}
          className="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          <Info className="size-3.5" />
        </PopoverTrigger>
        <PopoverContent
          align="end"
          className="flex w-72 flex-col gap-2 text-[13px]"
        >
          {explain && <div className="leading-snug">{explain}</div>}
          {meta && (
            <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
              <dt className="text-muted-foreground">
                {t('insights.status.computed')}
              </dt>
              <dd>
                {meta.generatedAt
                  ? format.relative(meta.generatedAt)
                  : t('insights.status.never')}
              </dd>
              {meta.window.kind === 'date-range' &&
                meta.window.dateRange.start && (
                  <>
                    <dt className="text-muted-foreground">
                      {t('insights.status.window')}
                    </dt>
                    <dd className="tabular">
                      {meta.window.dateRange.start} –{' '}
                      {meta.window.dateRange.end}
                    </dd>
                  </>
                )}
              <dt className="text-muted-foreground">
                {t('insights.status.state')}
              </dt>
              <dd>{t(`insights.status.states.${meta.state}`)}</dd>
            </dl>
          )}
          {limited && meta?.stateReason && (
            <p className="text-xs text-muted-foreground">{meta.stateReason}</p>
          )}
        </PopoverContent>
      </Popover>
    </span>
  )
}

/**
 * The body of a card backed by one query: an error with retry, a skeleton,
 * an honest empty line (or "updating" while the section is stale), or the
 * content.
 */
export function QueryBody<T>({
  query,
  skeleton,
  isEmpty,
  empty,
  children,
}: {
  query: UseQueryResult<T>
  skeleton: ReactNode
  isEmpty: (data: T) => boolean
  empty: string
  children: (data: T) => ReactNode
}) {
  const { t } = useI18n()
  if (query.isError && query.data === undefined) {
    return (
      <CardError error={query.error} onRetry={() => void query.refetch()} />
    )
  }
  if (query.data === undefined) return <>{skeleton}</>
  if (isEmpty(query.data)) {
    const stale = sectionMeta(query.data)?.state === 'stale'
    return (
      <p className="text-sm text-muted-foreground">
        {stale ? t('insights.stale') : empty}
      </p>
    )
  }
  return <>{children(query.data)}</>
}

function sectionMeta(data: unknown) {
  if (data && typeof data === 'object' && 'meta' in data) {
    return (data as Section<unknown>).meta
  }
  return null
}

/** Header for a drill-in: back to Insights, what this is, and actions. */
export function DrillHeader({
  back,
  eyebrow,
  title,
  subtitle,
  icon,
  actions,
}: {
  back: string
  eyebrow: string
  title: ReactNode
  subtitle?: ReactNode
  icon?: ReactNode
  actions?: ReactNode
}) {
  const { t } = useI18n()
  return (
    <header className="flex flex-col gap-3">
      <Link
        to={back}
        className="flex w-fit items-center gap-1.5 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
      >
        <ArrowLeft className="size-3.5" aria-hidden />
        {t('insights.title')}
      </Link>
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="flex min-w-0 items-center gap-3">
          {icon}
          <div className="flex min-w-0 flex-col gap-1">
            <span className="text-[13px] text-muted-foreground">{eyebrow}</span>
            <h1 className="truncate text-[28px] leading-tight font-semibold tracking-[-0.02em]">
              {title}
            </h1>
            {subtitle && (
              <p className="truncate text-[13px] text-muted-foreground">
                {subtitle}
              </p>
            )}
          </div>
        </div>
        {actions && (
          <div className="flex flex-wrap items-center gap-2.5">{actions}</div>
        )}
      </div>
    </header>
  )
}

export function RangeToggle({
  value,
  onChange,
}: {
  value: RangeId
  onChange: (range: RangeId) => void
}) {
  const { t } = useI18n()
  return (
    <ToggleGroup
      type="single"
      value={value}
      aria-label={t('insights.range.label')}
      onValueChange={(next) => isRangeId(next) && onChange(next)}
      className="rounded-[9px] bg-muted p-[3px]"
    >
      {ranges.map((id) => (
        <ToggleGroupItem
          key={id}
          value={id}
          className="h-[30px] rounded-[7px] px-3 text-[13px] data-[state=on]:bg-card data-[state=on]:shadow-card"
        >
          {t(`insights.range.${id}`)}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  )
}
