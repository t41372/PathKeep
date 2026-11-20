/**
 * When, and who (`agent-history.md` §2.4 items 5–7): the weekly rhythm of
 * the messages you sent, the time you were there beside the agents'
 * time, and each tool's summed work time.
 */
import { useMemo } from 'react'
import { heatLevel } from '@/components/app/heat-level'
import { HeatLegend, Heatmap, type HeatCell } from '@/components/app/heatmap'
import { SectionCard } from '@/components/app/section-card'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import type {
  InsightsHarnessRow,
  InsightsOverview,
} from '@/lib/backend-client/agent-insights'
import { useI18n } from '@/lib/i18n'
import { useInsightsFormat } from '@/features/insights/zone'
import { TOOL_COLOR } from './colors'
import type { Scope } from './scope'
import { ShareList } from './share-list'
import { hours, spanKey } from './words'

const HOURS = Array.from({ length: 24 }, (_, hour) => hour)

export function RhythmCard({
  hourOfWeek,
  overview,
  scope,
}: {
  hourOfWeek: number[][]
  overview: InsightsOverview
  scope: Scope
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const longNames = t('agentInsightsCards.rhythm.weekdays')
  const shortNames = t('insights.rhythm.weekdays')
  const { columns, peak, any } = useMemo(() => {
    const days = longNames.split(',')
    const max = Math.max(0, ...hourOfWeek.flat())
    const flat = HOURS.flatMap((hour) =>
      Array.from({ length: 7 }, (_, day) => ({
        hour,
        day,
        count: hourOfWeek[hour]?.[day] ?? 0,
      })),
    )
    const best = flat.reduce((top, cell) =>
      cell.count > top.count ? cell : top,
    )
    const columns = HOURS.map((hour): HeatCell[] =>
      Array.from({ length: 7 }, (_, day) => {
        const count = hourOfWeek[hour]?.[day] ?? 0
        return {
          key: `${hour}-${day}`,
          level: heatLevel(count, max),
          label: t('agentInsightsCards.rhythm.cell', {
            day: days[day] ?? '',
            hours: f.hourSpan(hour),
            messages: t('agentInsightsCards.rhythm.messages', { count }),
          }),
        }
      }),
    )
    return {
      columns,
      any: max > 0,
      peak: { day: days[best.day] ?? '', hour: f.clockTime(best.hour) },
    }
  }, [hourOfWeek, longNames, t, f])

  return (
    <SectionCard
      title={t('agentInsightsCards.rhythm.title')}
      subtitle={t('agentInsightsCards.rhythm.subtitle', {
        span: t(spanKey(scope.range)),
      })}
    >
      <div className="overflow-x-auto">
        <Heatmap
          columns={columns}
          label={t('agentInsightsCards.rhythm.grid')}
          rowLabels={shortNames.split(',')}
          columnLabels={HOURS.map((hour) =>
            hour % 6 === 0 || hour === 23 ? String(hour) : null,
          )}
        />
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
        <span>
          {any
            ? t('agentInsightsCards.rhythm.peak', peak)
            : t('agentInsightsCards.rhythm.empty')}
        </span>
        <HeatLegend
          less={t('agentInsights.year.less')}
          more={t('agentInsights.year.more')}
        />
      </div>
      {overview.undatedHumanMessages > 0 && (
        <p className="text-xs text-muted-foreground">
          {t('agentInsightsCards.rhythm.undated', {
            count: overview.undatedHumanMessages,
          })}
        </p>
      )}
    </SectionCard>
  )
}

export function PresenceCard({ overview }: { overview: InsightsOverview }) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const time = (ms: number | null) =>
    ms === null ? t('agentInsightsCards.noRecord') : hours(t, f, ms)
  return (
    <SectionCard
      title={t('agentInsightsCards.presence.title')}
      subtitle={t('agentInsights.how.present')}
    >
      <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
        <span className="text-2xl font-semibold tracking-[-0.02em] tabular">
          {overview.presentMs === null ? '—' : hours(t, f, overview.presentMs)}
        </span>
        <span className="text-xs text-muted-foreground">
          {t('agentInsightsCards.presence.beside', {
            time: time(overview.agentBusyMs),
          })}
        </span>
      </div>
      <p className="text-[13px] leading-relaxed text-muted-foreground">
        {t('agentInsightsCards.presence.note')}
      </p>
    </SectionCard>
  )
}

export function ToolTimeCard({ rows }: { rows: InsightsHarnessRow[] }) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  return (
    <SectionCard
      title={t('agentInsightsCards.tools.title')}
      subtitle={t('agentInsights.how.summed')}
    >
      <ShareList
        rows={rows.map((row) => ({
          key: row.harness,
          label: <ToolBadge id={row.harness} />,
          value: row.summedMs,
          display:
            row.summedMs === null
              ? t('agentInsightsCards.noRecord')
              : hours(t, f, row.summedMs),
          sub:
            row.conversationsWithDuration < row.conversations
              ? t('agentInsightsCards.projects.timed', {
                  known: f.number(row.conversationsWithDuration),
                  count: row.conversations,
                })
              : t('agentInsightsCards.conversations', {
                  count: row.conversations,
                }),
          color: TOOL_COLOR[row.harness],
        }))}
      />
    </SectionCard>
  )
}
