/**
 * "Agent work over time" (`agent-history.md` §2.4 item 1): for each bar of
 * the range, the clock time agents worked (at the same time counts once)
 * beside the summed time (each conversation counts its own). Only time:
 * tokens have their own card. Bars split at local midnight and at the
 * bucket edges (§1.3). A day's bar opens that day's conversations.
 */
import { useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import type { MouseHandlerDataParam } from 'recharts'
import { SectionCard } from '@/components/app/section-card'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import type {
  InsightsSeries,
  InsightsWork,
} from '@/lib/backend-client/agent-insights'
import { useI18n } from '@/lib/i18n'
import { dateKeyIn } from '@/lib/time-zone'
import { useInsightsFormat } from '@/features/insights/zone'
import { BucketTooltip, type BucketWords } from './bucket-tooltip'
import { BESIDE_COLOR, MAIN_COLOR } from './colors'
import { Legend } from './legend'
import { historyOnDay, type Scope } from './scope'
import type { Shown } from './use-report'
import {
  RANGE_UNIT,
  bucketTick,
  bucketTitle,
  hours,
  perKey,
  spanKey,
} from './words'

interface Row extends Record<string, unknown> {
  key: string
  start: number
  end: number
  busy: number | null
  summed: number | null
  busyMs: number | null
  summedMs: number | null
  conversations: number | null
}

const toHours = (ms: number | null) =>
  ms === null ? null : Math.round((ms / 3_600_000) * 100) / 100

export function WorkCard({
  series,
  work,
  scope,
  zone,
}: {
  series: Shown<InsightsSeries>
  work: InsightsWork | null
  scope: Scope
  zone: string
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const navigate = useNavigate()
  const source = series?.value ?? work
  const unit = source?.unit ?? RANGE_UNIT[scope.range]
  const rows = useMemo<Row[]>(() => {
    const row = (
      bucket: InsightsWork['buckets'][number],
      conversations: number | null,
    ): Row => ({
      key: String(bucket.bucketStart),
      start: bucket.bucketStart,
      end: bucket.bucketEnd,
      busy: toHours(bucket.busyMs),
      summed: toHours(bucket.summedMs),
      busyMs: bucket.busyMs,
      summedMs: bucket.summedMs,
      conversations,
    })
    if (series)
      return series.value.buckets.map((b) => row(b, b.conversationCount))
    return (work?.buckets ?? []).map((b) => row(b, null))
  }, [series, work])
  const config = useMemo(
    () => ({
      busy: {
        label: t('agentInsightsCards.work.busyShort'),
        colors: { light: [MAIN_COLOR], dark: [MAIN_COLOR] },
      },
      summed: {
        label: t('agentInsightsCards.work.summedShort'),
        colors: { light: [BESIDE_COLOR], dark: [BESIDE_COLOR] },
      },
    }),
    [t],
  )
  const time = (ms: number | null) =>
    ms === null ? t('agentInsightsCards.noRecord') : hours(t, f, ms)
  const describe = (row: Row): BucketWords => ({
    title: bucketTitle(t, f, unit, row.start, row.end, zone),
    lines: [
      {
        key: 'busy',
        label: t('agentInsightsCards.work.busyShort'),
        color: MAIN_COLOR,
        value: time(row.busyMs),
      },
      {
        key: 'summed',
        label: t('agentInsightsCards.work.summedShort'),
        color: BESIDE_COLOR,
        value: time(row.summedMs),
      },
    ],
    note:
      row.conversations === null
        ? null
        : t('agentInsightsCards.conversations', { count: row.conversations }),
  })
  const openDay = (state: MouseHandlerDataParam) => {
    if (unit !== 'day' || state.activeLabel === undefined) return
    void navigate(
      historyOnDay(scope, dateKeyIn(Number(state.activeLabel), zone)),
    )
  }

  return (
    <SectionCard
      title={t('agentInsightsCards.work.title')}
      subtitle={t('agentInsightsCards.work.subtitle', {
        span: t(spanKey(scope.range)),
        per: t(perKey(unit)),
      })}
    >
      <Legend
        items={[
          { label: t('agentInsightsCards.work.busy'), color: MAIN_COLOR },
          { label: t('agentInsightsCards.work.summed'), color: BESIDE_COLOR },
        ]}
      />
      <div
        role="img"
        aria-label={t('agentInsightsCards.work.chart', {
          per: t(perKey(unit)),
        })}
        className="h-[230px]"
      >
        <EvilBarChart
          data={rows}
          config={config}
          xDataKey="key"
          isLoading={!source}
          className="aspect-auto h-full"
          barRadius={2}
          barGap={1}
          chartProps={{
            onClick: openDay,
            style: unit === 'day' ? { cursor: 'pointer' } : undefined,
          }}
        >
          <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
          <EvilBarChart.XAxis
            dataKey="key"
            minTickGap={28}
            tickFormatter={(value: string) =>
              bucketTick(f, unit, Number(value), zone)
            }
            className="font-mono"
          />
          <EvilBarChart.YAxis
            tickFormatter={(value: number) => f.number(value)}
            className="font-mono"
            width={40}
          />
          <BucketTooltip<Row> describe={describe} />
          <EvilBarChart.Bar dataKey="busy" />
          <EvilBarChart.Bar dataKey="summed" />
        </EvilBarChart>
      </div>
    </SectionCard>
  )
}
