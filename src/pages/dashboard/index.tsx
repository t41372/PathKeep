/**
 * Dashboard route — paper redesign.
 *
 * Why this file exists:
 * - Renders the v0.3 paper Dashboard: greeting band, On This Day hero, This
 *   Week summary, year heatmap, active threads, archive card, footer.
 * - Falls back to the existing fallback / zero-state paths when the archive
 *   is locked, errored, or freshly initialized.
 *
 * Responsibilities:
 * - Compose the paper-aesthetic landing page.
 * - Pull data from useShellData + the existing core-intelligence API.
 * - Wire deep links into Explorer, Intelligence, Settings.
 *
 * Not responsible for:
 * - Backup orchestration (lives in the shell).
 * - Detailed intelligence calls beyond the dashboard summary (delegated to
 *   the Intelligence route's deep components).
 */

import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useShellData } from '@/app/shell-data-context'
import { useI18n } from '@/lib/i18n'
import { describeError } from '@/lib/errors'
import * as coreIntelligenceApi from '@/lib/core-intelligence/api'
import {
  buildIntelligenceSearchParams,
  dateRangeFromPreset,
  domainInsightsHref,
} from '@/lib/core-intelligence'
import type { OnThisDayEntry, PathFlow } from '@/lib/core-intelligence/types'
import { useProfileScope } from '@/lib/profile-scope-context'
import { DashboardArchiveCard } from './archive-card'
import { DashboardOnThisDay } from './on-this-day-card'
import { DashboardThisWeek } from './this-week-card'
import { DashboardActiveThreads } from './active-threads-card'
import {
  compactNumber,
  firstRegistrableDomainStep,
  formatSpan,
  humanizeBytes,
  sumStorageBytes,
} from './dashboard-helpers'
import { useDashboardArchiveAccessFallback } from './route-fallback-access'
import { DashboardRouteFallback } from './route-fallback'
import { resolveDashboardRouteFallback } from './route-fallback-state'
import { DashboardYearHeatmapCard } from './year-heatmap-card'
import { cn } from '@/lib/cn'

export function DashboardPage() {
  const {
    dashboard,
    dashboardLoading = false,
    error,
    loading,
    refreshKey,
    snapshot,
  } = useShellData()
  const { t } = useI18n()
  const { activeProfileId } = useProfileScope()
  const navigate = useNavigate()

  const [onThisDayEntries, setOnThisDayEntries] = useState<OnThisDayEntry[]>([])
  const [onThisDayLoading, setOnThisDayLoading] = useState(false)
  const [onThisDayError, setOnThisDayError] = useState<string | null>(null)

  const archiveAccessFallback = useDashboardArchiveAccessFallback({
    dashboard,
    error,
    refreshKey,
    snapshot,
  })

  const greeting = useMemoGreeting(t)

  useEffect(() => {
    if (!snapshot?.config.initialized) {
      setOnThisDayLoading(false)
      return
    }
    let cancelled = false
    setOnThisDayLoading(true)
    void (async () => {
      try {
        const result = await coreIntelligenceApi.getOnThisDay(activeProfileId)
        if (!cancelled) {
          setOnThisDayEntries(result.data ?? [])
          setOnThisDayError(null)
        }
      } catch (nextError) {
        if (!cancelled) {
          setOnThisDayEntries([])
          setOnThisDayError(describeError(nextError, 'get_on_this_day'))
        }
      } finally {
        if (!cancelled) {
          setOnThisDayLoading(false)
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [activeProfileId, refreshKey, snapshot?.config.initialized, t])

  const fallbackState = resolveDashboardRouteFallback({
    archiveAccessFallback,
    dashboard,
    dashboardLoading,
    error,
    loading,
    snapshot,
  })

  if (fallbackState.kind !== 'ready') {
    return <DashboardRouteFallback state={fallbackState} t={t} />
  }

  const readySnapshot = snapshot!
  const readyDashboard = dashboard!

  const totalStorageBytes = sumStorageBytes(readyDashboard.storage)
  const totalSizeLabel = humanizeBytes(totalStorageBytes)
  const spanLabel = readyDashboard.earliestVisitAt
    ? formatSpan(
        readyDashboard.earliestVisitAt,
        t,
        readyDashboard.latestVisitAt ?? null,
      )
    : '—'
  const sourcesCount = readySnapshot.browserProfiles.length

  return (
    <div
      className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
      data-testid="dashboard-page"
    >
      <HeroBand
        greeting={greeting}
        message={t('dashboard.heroMessage')}
        stats={[
          {
            value: compactNumber(readyDashboard.totalVisits),
            label: t('dashboard.statPages'),
          },
          { value: spanLabel, label: t('dashboard.statSpan') },
          { value: totalSizeLabel || '0 B', label: t('dashboard.statSize') },
          { value: String(sourcesCount), label: t('dashboard.statSources') },
        ]}
      />

      <div className="grid grid-cols-1 gap-4 mb-4 lg:grid-cols-2">
        <DashboardOnThisDay
          entries={onThisDayEntries}
          loading={onThisDayLoading}
          error={onThisDayError}
          onJumpToDate={(date) =>
            void navigate(
              `/explorer?date=${encodeURIComponent(date)}&source=on-this-day`,
            )
          }
          onOpenEntry={(entry) =>
            void navigate(
              `/explorer?date=${encodeURIComponent(entry.date)}&source=on-this-day`,
            )
          }
        />
        <DashboardThisWeek
          archiveReady={
            readySnapshot.config.initialized &&
            readySnapshot.archiveStatus.unlocked
          }
          recentRuns={readyDashboard.recentRuns}
        />
      </div>

      <DashboardYearHeatmapCard
        archiveReady={
          readySnapshot.config.initialized &&
          readySnapshot.archiveStatus.unlocked
        }
        onOpenInsights={() => void navigate('/intelligence')}
        onSelectDate={(date) =>
          void navigate(
            `/explorer?date=${encodeURIComponent(date)}&source=on-this-day`,
          )
        }
      />

      <div className="grid grid-cols-1 gap-4 mb-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <DashboardActiveThreads
            archiveReady={
              readySnapshot.config.initialized &&
              readySnapshot.archiveStatus.unlocked
            }
            onOpenAll={() => void navigate('/intelligence')}
            onOpenThread={(flow) => void navigate(pathFlowDeepLink(flow))}
          />
        </div>
        <DashboardArchiveCard
          databasePath={readySnapshot.archiveStatus.databasePath}
          archiveMode={readySnapshot.config.archiveMode}
          totalBytes={totalStorageBytes}
          storage={readyDashboard.storage}
          latestManifestHash={
            readyDashboard.recentRuns.find((run) => run.manifestHash)
              ?.manifestHash ?? null
          }
        />
      </div>

      <FooterEpigraph>{t('dashboard.localFirstFooter')}</FooterEpigraph>
    </div>
  )
}

interface HeroBandProps {
  greeting: string
  message: string
  stats: { value: string; label: string }[]
}

function HeroBand({ greeting, message, stats }: HeroBandProps) {
  return (
    <header
      className={cn(
        'grid grid-cols-1 items-end gap-10 border-b border-border-light pb-5 mb-7',
        'lg:grid-cols-[1fr_auto]',
      )}
    >
      <div>
        <h1 className="mb-[6px] font-serif text-[26px] font-normal leading-[1.2] tracking-[-0.02em] text-ink">
          {greeting}
        </h1>
        <p className="m-0 max-w-[500px] font-serif text-[15px] italic leading-[1.5] text-ink-muted">
          {message}
        </p>
      </div>
      <div className="flex flex-wrap items-end gap-7">
        {stats.map((stat) => (
          <div key={stat.label} className="text-center">
            <div className="font-serif text-[24px] font-normal leading-none tracking-[-0.02em] text-ink">
              {stat.value}
            </div>
            <div className="mt-1 font-mono text-[9px] uppercase tracking-[0.08em] text-ink-faint">
              {stat.label}
            </div>
          </div>
        ))}
      </div>
    </header>
  )
}

function FooterEpigraph({ children }: { children: string }) {
  return (
    <div className="border-border-light mt-6 border-t pt-5 text-center font-serif text-[13px] italic text-ink-faint pb-2">
      {children}
    </div>
  )
}

function useMemoGreeting(
  t: (key: string, vars?: Record<string, string | number>) => string,
): string {
  return useMemo(() => {
    const hour = new Date().getHours()
    if (hour < 12) return t('dashboard.greetingMorning')
    if (hour < 18) return t('dashboard.greetingAfternoon')
    return t('dashboard.greetingEvening')
  }, [t])
}

/**
 * Builds the deep link that opens a clicked Active Threads flow somewhere that
 * actually surfaces it.
 *
 * The `/intelligence` index route parses `focusType=path-flow` but discards it —
 * landing there is indistinguishable from "See all". The domain deep-dive
 * (`/intelligence/domain/:domain`) is the route that genuinely consumes a
 * path-flow focus: it loads the flows, finds the focused one, and renders the
 * "Focused path flow" callout when that flow touches the page's domain. So we
 * route to the flow's first registrable-domain step carrying the focus — the
 * domain comes from the flow itself, which guarantees the callout matches.
 *
 * A flow whose steps are all non-domain groups has no domain page to land on;
 * it falls back to the overview deep-link (still month-scoped and focus-tagged)
 * because there is no better honest destination for a domain-less flow.
 *
 * The month preset is the closest range to the card's 30-day query window.
 */
function pathFlowDeepLink(flow: PathFlow): string {
  const focus = { focusType: 'path-flow' as const, focusId: flow.flowId }
  const focusDomain = firstRegistrableDomainStep(flow)
  if (focusDomain) {
    return domainInsightsHref({
      domain: focusDomain,
      dateRange: dateRangeFromPreset('month'),
      preset: 'month',
      focus,
    })
  }
  const params = buildIntelligenceSearchParams({
    dateRange: dateRangeFromPreset('month'),
    preset: 'month',
    focus,
  })
  return `/intelligence?${params.toString()}`
}
