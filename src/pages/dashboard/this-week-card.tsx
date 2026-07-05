/**
 * @file this-week-card.tsx
 * @description Dashboard "This week" summary card. Wired to the deterministic
 * discovery-trend backend so the stats under the week badge reflect actual
 * browsing in the current ISO week instead of archive-wide totals.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Fetch the current ISO-week (Mon→Sun) discovery-trend points for the active
 *   profile and present this-week visit + new-domain counts.
 * - Count how many backup runs started this week from the snapshot the parent
 *   already holds (no extra backend round-trip).
 * - Surface honest loading / empty / error states without forging an editorial
 *   summary when no local LLM is configured (Trust & Transparency).
 *
 * ## Not responsible for
 * - Deep-link routing — the card is a leaf summary with no navigation affordance.
 * - Owning the week-window or sum math — `dashboard-helpers` holds those pure
 *   transforms so this file stays a thin render + fetch shell.
 *
 * ## Dependencies
 * - `coreIntelligenceApi.getDiscoveryTrend` for the weekly visit/domain counts.
 * - `useProfileScope` for the active profile.
 * - `dashboardWeekRange` / `sumWeekTrend` / `countRunsInRange` for pure math.
 *
 * ## Performance notes
 * - Only fetches when the archive is initialized + unlocked; otherwise stays
 *   idle so the card never fires backend queries before the first run. The
 *   discovery-trend read is a bounded 7-day window, so it stays cheap even when
 *   the archive holds millions of visits.
 */

import { useEffect, useState } from 'react'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { Skeleton } from '@/components/primitives/skeleton'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import * as coreIntelligenceApi from '@/lib/core-intelligence/api'
import { useProfileScope } from '@/lib/profile-scope-context'
import type { BackupRunOverview } from '@/lib/types'
import {
  countRunsInRange,
  dashboardWeekRange,
  sumWeekTrend,
} from './dashboard-helpers'

export interface DashboardThisWeekProps {
  /** Whether the archive is ready to be queried (initialized + unlocked). */
  archiveReady: boolean
  /**
   * Recent backup runs from the dashboard snapshot. Filtered to the current
   * week locally so the "Runs" stat counts this week's runs, not the all-time
   * recent slice.
   */
  recentRuns: BackupRunOverview[]
}

export function DashboardThisWeek({
  archiveReady,
  recentRuns,
}: DashboardThisWeekProps) {
  const { t, language } = useI18n()
  const { activeProfileId } = useProfileScope()
  const weekNumber = isoWeek(new Date())

  const [totals, setTotals] = useState({ totalVisits: 0, newDomains: 0 })
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!archiveReady) {
      setTotals({ totalVisits: 0, newDomains: 0 })
      setLoading(false)
      setError(null)
      return
    }
    let cancelled = false
    setLoading(true)
    void (async () => {
      try {
        const range = dashboardWeekRange(new Date())
        const result = await coreIntelligenceApi.getDiscoveryTrend(
          range,
          activeProfileId,
          'day',
        )
        if (!cancelled) {
          setTotals(sumWeekTrend(result.data?.points))
          setError(null)
        }
      } catch (nextError) {
        if (!cancelled) {
          setTotals({ totalVisits: 0, newDomains: 0 })
          setError(describeError(nextError, 'get_discovery_trend'))
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [activeProfileId, archiveReady, t])

  const runsThisWeek = countRunsInRange(
    recentRuns,
    dashboardWeekRange(new Date()),
  )
  const fmt = (value: number): string =>
    new Intl.NumberFormat(language === 'en' ? 'en-US' : language).format(value)

  return (
    <PaperCard testId="dashboard-this-week">
      <PaperCardHeader
        title={t('dashboard.thisWeekTitle')}
        right={
          <PaperCardBadge>
            {t('dashboard.weekBadge', { week: weekNumber })}
          </PaperCardBadge>
        }
      />
      <PaperCardBody className="px-[18px] py-[14px]">
        <p className="m-0 mb-2 font-serif text-[14px] leading-[1.55] text-ink-secondary">
          {t('dashboard.thisWeekFallbackHeadline')}
        </p>
        <p className="m-0 font-serif text-[13px] leading-[1.55] text-ink-faint italic">
          {t('dashboard.thisWeekFallbackHint')}
        </p>

        {loading ? (
          <div
            className="border-border-light mt-[14px] flex gap-0 border-t border-dashed pt-3"
            data-testid="dashboard-this-week-loading"
            aria-busy="true"
            aria-label={t('common.loading')}
          >
            {[0, 1, 2].map((index) => (
              <div key={index} className="flex-1">
                <Skeleton className="h-[20px] w-10" />
                <Skeleton className="mt-1 h-[8.5px] w-14" />
              </div>
            ))}
          </div>
        ) : error ? (
          <p
            className="border-border-light mt-[14px] m-0 border-t border-dashed pt-3 font-serif text-[13px] italic leading-[1.55] text-danger"
            data-testid="dashboard-this-week-error"
          >
            {error}
          </p>
        ) : (
          <div
            className="border-border-light mt-[14px] flex gap-0 border-t border-dashed pt-3"
            data-testid="dashboard-this-week-stats"
          >
            {[
              {
                val: fmt(totals.totalVisits),
                label: t('dashboard.weekStatPages'),
              },
              {
                val: fmt(totals.newDomains),
                label: t('dashboard.weekStatSites'),
              },
              {
                val: fmt(runsThisWeek),
                label: t('dashboard.weekStatRuns'),
              },
            ].map((stat) => (
              <div key={stat.label} className="flex-1">
                <div className="font-serif text-[20px] font-normal tracking-[-0.01em] text-ink">
                  {stat.val}
                </div>
                <div className="mt-0.5 font-mono text-[8.5px] uppercase tracking-[0.06em] text-ink-faint">
                  {stat.label}
                </div>
              </div>
            ))}
          </div>
        )}
      </PaperCardBody>
    </PaperCard>
  )
}

function isoWeek(date: Date): number {
  const target = new Date(date.valueOf())
  target.setHours(0, 0, 0, 0)
  target.setDate(target.getDate() + 4 - (target.getDay() || 7))
  const firstThursday = new Date(target.getFullYear(), 0, 4)
  const diff = (target.getTime() - firstThursday.getTime()) / 86400000
  return 1 + Math.round(diff / 7)
}
