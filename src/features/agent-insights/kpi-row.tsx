/**
 * The five numbers under the title (`agent-history.md` §2.2): Agent work
 * time, the time you were there, tokens, cost and conversations, each with
 * the line under it and a note on how it is counted. Without "Show cost"
 * there are four. A number nothing recorded reads "—", never 0, and the
 * line under it says how many conversations have it.
 */
import { Info } from 'lucide-react'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import type {
  EquivalentReading,
  InsightsOverview,
} from '@/lib/backend-client/agent-insights'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useInsightsFormat } from '@/features/insights/zone'
import { Equivalent, HoursWithEquivalent } from './equivalent'
import { historyInRange, type Scope } from './scope'
import { hours, tokenTotal, usd } from './words'

const NONE = '—'

function Tile({
  label,
  value,
  sub,
  how,
  to,
  name,
}: {
  label: string
  value: ReactNode
  sub: ReactNode
  how?: ReactNode
  to?: string
  /** The link's name, when the tile is one. */
  name?: string
}) {
  const { t } = useI18n()
  const body = (
    <>
      <span className="flex items-center gap-1 text-[13px] text-muted-foreground">
        {label}
        {how && (
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-label={t('agentInsights.kpi.about')}
                className="inline-flex rounded-sm text-muted-foreground/70 outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
              >
                <Info className="size-3.5" aria-hidden />
              </button>
            </TooltipTrigger>
            <TooltipContent className="flex max-w-[300px] flex-col gap-1.5 leading-relaxed">
              {how}
            </TooltipContent>
          </Tooltip>
        )}
      </span>
      <span className="text-2xl font-semibold tracking-[-0.02em] tabular">
        {value}
      </span>
      <span className="text-xs text-muted-foreground">{sub}</span>
    </>
  )
  const frame =
    'flex min-w-0 flex-col gap-1.5 rounded-xl border bg-card px-[18px] py-4 shadow-card'
  return to ? (
    <Link
      to={to}
      aria-label={name}
      className={cn(
        frame,
        'transition-colors hover:bg-card/80 hover:no-underline',
      )}
    >
      {body}
    </Link>
  ) : (
    <div role="group" aria-label={label} className={frame}>
      {body}
    </div>
  )
}

export function KpiRow({
  overview,
  scope,
  reading,
  onReading,
}: {
  overview: InsightsOverview
  scope: Scope
  reading: EquivalentReading
  onReading: (next: EquivalentReading) => void
}) {
  const { t, locale } = useI18n()
  const f = useInsightsFormat()
  const time = (ms: number | null) => (ms === null ? NONE : hours(t, f, ms))
  const n = overview.conversationCount
  const equivalent = (ms: number | null) =>
    ms === null ? null : (
      <Equivalent ms={ms} reading={reading} onSwitch={onReading} />
    )
  const total = overview.tokens ? tokenTotal(overview.tokens) : null
  const cost = overview.cost
  const money = (value: number | null) =>
    value === null ? NONE : usd(value, locale)

  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(190px,1fr))] gap-3">
      <Tile
        label={t('agentInsights.kpi.agent')}
        value={
          <HoursWithEquivalent equivalent={equivalent(overview.agentBusyMs)}>
            {time(overview.agentBusyMs)}
          </HoursWithEquivalent>
        }
        sub={
          overview.concurrency === null
            ? t('agentInsights.kpi.agentSubSummed', {
                summed: time(overview.agentSummedMs),
              })
            : t('agentInsights.kpi.agentSub', {
                summed: time(overview.agentSummedMs),
                concurrency: f.number(
                  Math.round(overview.concurrency * 10) / 10,
                ),
              })
        }
        how={
          <>
            <span>{t('agentInsights.how.agent')}</span>
            <span>{t('agentInsights.how.summed')}</span>
            <span>
              {t('agentInsights.how.durations', {
                count: n,
                recorded: f.number(overview.recordedDurationConversations),
              })}
            </span>
            {overview.unknownDurationConversations > 0 && (
              <span>
                {t('agentInsights.how.noDuration', {
                  count: overview.unknownDurationConversations,
                })}
              </span>
            )}
          </>
        }
      />
      <Tile
        label={t('agentInsights.kpi.present')}
        value={
          <HoursWithEquivalent equivalent={equivalent(overview.presentMs)}>
            {time(overview.presentMs)}
          </HoursWithEquivalent>
        }
        sub={t('agentInsights.kpi.presentSub')}
        how={t('agentInsights.how.present')}
      />
      <Tile
        label={t('agentInsights.kpi.tokens')}
        value={total === null ? NONE : f.compact(total)}
        sub={
          overview.conversationsWithUsage < n || total === null
            ? t('agentInsights.coverage', {
                count: n,
                known: f.number(overview.conversationsWithUsage),
              })
            : t('agentInsights.kpi.cacheShare', {
                share: f.percent(
                  total > 0 ? overview.tokens!.cacheRead / total : 0,
                ),
              })
        }
        how={t('agentInsights.how.tokens')}
      />
      {cost && (
        <Tile
          label={t('agentInsights.kpi.cost')}
          value={money(cost.totalUsd)}
          sub={
            cost.includeEstimates
              ? t('agentInsights.kpi.costBoth', {
                  recorded: money(cost.recordedUsd),
                  estimated: money(cost.estimatedUsd),
                })
              : t('agentInsights.kpi.costRecorded', {
                  count: cost.recordedConversations,
                })
          }
          how={t('agentInsights.how.cost')}
        />
      )}
      <Tile
        label={t('agentInsights.kpi.conversations')}
        value={f.number(n)}
        sub={t('agentInsights.kpi.conversationsSub', {
          subagents: t('agentInsights.kpi.subagents', {
            count: overview.subagentCount,
          }),
          projects: t('agentInsights.kpi.projects', {
            count: overview.projectCount,
          }),
        })}
        to={historyInRange(scope)}
        name={t('agentInsights.kpi.openList', { count: n })}
      />
    </div>
  )
}
