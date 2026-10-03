/**
 * Home: a summary of the archive. Header with backup status, four stat
 * cards, the 30-day trend, on-this-day, the year heatmap, recurring topics
 * and sources.
 */
import { Link } from 'react-router-dom'
import {
  BackupNowButton,
  BackupStatusPill,
} from '@/components/app/backup-button'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { Skeleton } from '@/components/ui/skeleton'
import { useFormat, useI18n, type Translator } from '@/lib/i18n'
import { useDashboard, useSnapshot } from '@/lib/queries/app'
import {
  OnThisDayCard,
  SourcesCard,
  ThreadsCard,
  TrendCard,
  YearCard,
} from './home-cards'
import { dailySeries, useBrowserSummaries } from './home-data'
import { useThirtyDayTrend } from './queries'

function greeting(t: Translator, hour = new Date().getHours()) {
  if (hour < 5) return t('home.greetingEvening')
  if (hour < 12) return t('home.greetingMorning')
  if (hour < 18) return t('home.greetingAfternoon')
  return t('home.greetingEvening')
}

function timeSpan(t: Translator, earliest: string) {
  const start = new Date(earliest)
  const now = new Date()
  const months =
    (now.getFullYear() - start.getFullYear()) * 12 +
    now.getMonth() -
    start.getMonth()
  if (months >= 12) {
    return t('home.stats.spanYearsMonths', {
      years: Math.floor(months / 12),
      months: months % 12,
    })
  }
  if (months >= 1) return t('home.stats.spanMonths', { count: months })
  const days = Math.max(
    1,
    Math.round((now.getTime() - start.getTime()) / 86_400_000),
  )
  return t('home.stats.spanDays', { count: days })
}

function StatCard({
  to,
  label,
  value,
  sub,
  loading,
}: {
  to: string
  label: string
  value: string
  sub: string
  loading?: boolean
}) {
  return (
    <Link
      to={to}
      className="flex flex-col gap-1.5 rounded-xl border bg-card px-[18px] py-4 shadow-card transition-colors hover:bg-card/90 hover:no-underline"
    >
      <span className="text-[13px] text-muted-foreground">{label}</span>
      {loading ? (
        <Skeleton className="my-1 h-6 w-28" />
      ) : (
        <span className="text-2xl font-semibold tracking-[-0.02em] tabular">
          {value}
        </span>
      )}
      <span className="text-xs text-muted-foreground">{sub}</span>
    </Link>
  )
}

function StatCards() {
  const { t } = useI18n()
  const format = useFormat()
  const snapshot = useSnapshot()
  const dashboard = useDashboard()
  const trend = useThirtyDayTrend()
  const browsers = useBrowserSummaries()

  const data = dashboard.data
  const thisWeek = dailySeries(trend.data?.data.points ?? [], 7).reduce(
    (sum, day) => sum + day.visits,
    0,
  )
  const storage = data?.storage
  const archiveBytes = storage
    ? Object.values<number>({ ...storage }).reduce(
        (sum, value) => sum + value,
        0,
      )
    : 0
  const active = browsers.list.filter((item) => item.selected).length
  const paused = browsers.list.length - active

  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(180px,1fr))] gap-3">
      <StatCard
        to="/history"
        label={t('home.stats.saved')}
        value={format.number(data?.totalVisits ?? 0)}
        sub={t('home.stats.savedSub', { count: thisWeek })}
        loading={dashboard.isPending}
      />
      <StatCard
        to="/insights"
        label={t('home.stats.span')}
        value={data?.earliestVisitAt ? timeSpan(t, data.earliestVisitAt) : '—'}
        sub={
          data?.earliestVisitAt
            ? t('home.stats.spanSince', {
                date: format.date(data.earliestVisitAt, {
                  year: 'numeric',
                  month: 'long',
                }),
              })
            : ''
        }
        loading={dashboard.isPending}
      />
      <StatCard
        to="/settings/storage"
        label={t('home.stats.size')}
        value={format.bytes(archiveBytes)}
        sub={t(
          snapshot.archiveStatus.encrypted
            ? 'home.stats.encrypted'
            : 'home.stats.notEncrypted',
        )}
        loading={dashboard.isPending}
      />
      <StatCard
        to="/backup"
        label={t('home.stats.sources')}
        value={t('home.stats.sourcesValue', { count: active })}
        sub={
          paused > 0
            ? t('home.stats.sourcesPaused', { count: paused })
            : t('home.stats.sourcesAllOn')
        }
      />
    </div>
  )
}

export default function HomePage() {
  const { t } = useI18n()
  const format = useFormat()

  return (
    <PageScroll>
      <PageHeader
        eyebrow={format.weekdayDate(new Date())}
        title={t('home.title', { greeting: greeting(t) })}
        actions={
          <>
            <BackupStatusPill />
            <BackupNowButton />
          </>
        }
      />
      <StatCards />
      <div className="grid grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <TrendCard />
        <OnThisDayCard />
      </div>
      <YearCard />
      <div className="grid grid-cols-2 gap-4 max-[900px]:grid-cols-1">
        <ThreadsCard />
        <SourcesCard />
      </div>
    </PageScroll>
  )
}
