/**
 * When the user found new sites: sites visited for the first time per day,
 * week or month of the scope (in any browser, or in the one profile chosen),
 * with the total and, for a bounded range, what share of the range's sites
 * they were. Bars group like the activity chart (`buckets.ts`).
 */
import { useMemo } from 'react'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { CardError, SectionCard } from '@/components/app/section-card'
import { isStale } from '@/lib/backend-client/insights'
import { useI18n } from '@/lib/i18n'
import { barLabel, chartDates, rangeBars } from './buckets'
import { SectionStatus } from './parts'
import { useDigest, useNewSites } from './queries'
import { scopeDays, type Scope } from './range'
import { useInsightsFormat } from './zone'

export function NewSitesCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const newSites = useNewSites(scope)
  const digest = useDigest(scope)
  const { length, bars, total } = useMemo(() => {
    const byDay = new Map(
      (newSites.data?.data.points ?? []).map((point) => [
        point.dateKey,
        { sites: point.newDomainCount },
      ]),
    )
    let total = 0
    for (const day of byDay.values()) total += day.sites
    return {
      ...rangeBars(chartDates(scope, byDay.keys()), byDay, { sites: 0 }),
      total,
    }
  }, [newSites.data, scope])
  const config = useMemo(
    () => ({
      sites: {
        label: t('insights.newSites.series'),
        colors: { light: ['var(--green)'], dark: ['var(--green)'] },
      },
    }),
    [t],
  )

  // Over all time every site is new, so the share says nothing there.
  const distinct = digest.data?.data.distinctDomains.value ?? 0
  const share =
    scopeDays(scope) !== null && distinct > 0
      ? t('insights.newSites.share', {
          share: format.percent(Math.min(1, total / distinct)),
        })
      : null

  return (
    <SectionCard
      title={t('insights.newSites.title')}
      subtitle={
        newSites.data && total > 0
          ? [
              t('insights.newSites.total', {
                count: total,
                sites: format.number(total),
              }),
              share,
            ]
              .filter(Boolean)
              .join(' · ')
          : undefined
      }
      action={
        <SectionStatus
          meta={newSites.data?.meta}
          explain={t('insights.newSites.explain')}
        />
      }
    >
      <div className={total === 0 ? undefined : 'h-[180px]'}>
        {newSites.isError && newSites.data === undefined ? (
          <CardError
            error={newSites.error}
            onRetry={() => void newSites.refetch()}
          />
        ) : newSites.data && total === 0 ? (
          <p className="text-sm text-muted-foreground">
            {isStale(newSites.data)
              ? t('insights.stale')
              : t('insights.newSites.none')}
          </p>
        ) : (
          <EvilBarChart
            data={bars}
            config={config}
            xDataKey="dateKey"
            isLoading={newSites.data === undefined}
            className="aspect-auto h-full"
            barRadius={3}
          >
            <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
            <EvilBarChart.XAxis
              dataKey="dateKey"
              tickLine={false}
              axisLine={false}
              minTickGap={32}
              tickFormatter={(value: string) => barLabel(format, length, value)}
              className="font-mono"
            />
            <EvilBarChart.Tooltip />
            <EvilBarChart.Bar dataKey="sites" />
          </EvilBarChart>
        )}
      </div>
    </SectionCard>
  )
}
