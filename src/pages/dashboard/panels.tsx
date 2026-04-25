/**
 * @file panels.tsx
 * @description Owns the render-only dashboard panels so the route shell can stay focused on loading, gating, and composition.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Render the reusable panel sections that make up the dashboard body and zero state.
 * - Keep dashboard deep-link and trust-action chrome consistent across route branches.
 *
 * ## Not responsible for
 * - Fetching dashboard data or shell snapshots
 * - Deciding whether the route is loading, locked, unavailable, or in zero-state mode
 *
 * ## Dependencies
 * - Depends on shared primitives, browser-retention metadata, formatting helpers, and dashboard helper view models.
 *
 * ## Performance notes
 * - These components are render-only; keep them free of local effects so dashboard transitions stay cheap on large archives.
 */

import { Link } from 'react-router-dom'
import { BrowsingRhythmCard } from '../../components/intelligence/browsing-rhythm-card'
import { Skeleton } from '../../components/primitives/skeleton'
import { browserRetentionMeta } from '../../lib/browser-retention'
import { BrowserIcon } from '../../lib/browser-icons'
import {
  dayInsightsHref,
  domainDayInsightsHref,
} from '../../lib/core-intelligence/routes'
import { formatBytes, formatRelativeTime } from '../../lib/format'
import type { ResolvedLanguage } from '../../lib/i18n'
import type { BrowserProfile, DashboardSnapshot } from '../../lib/types'
import type { OnThisDayEntry } from '../../lib/core-intelligence/types'
import type { aiStatusMeta } from '../../lib/intelligence-ai-presentation'
import { runStatusKey, runTypeKey } from '../../lib/trust-review'
import type { DashboardStatItem, DashboardStorageSegment } from './helpers'

type Translate = (key: string, vars?: Record<string, string | number>) => string

interface DashboardStatsRowProps {
  stats: DashboardStatItem[]
}

/**
 * Renders the top-line stat cards shared by both the primary dashboard and its zero-state branch.
 */
export function DashboardStatsRow({ stats }: DashboardStatsRowProps) {
  return (
    <div className="stats-row">
      {stats.map((stat) => (
        <article key={stat.label} className="stat-card" data-tone={stat.tone}>
          <div className="stat-label">{stat.label}</div>
          <div className="stat-value">{stat.value}</div>
          <div
            className={`stat-delta ${stat.tone === 'success' ? 'positive' : 'neutral'}`}
          >
            {stat.detail}
          </div>
        </article>
      ))}
    </div>
  )
}

interface DashboardRecentRunsPanelProps {
  dashboard: DashboardSnapshot
  language: ResolvedLanguage
  runSourceSummary: (profileScope: string[] | undefined) => string
  t: Translate
}

/**
 * Keeps the recent-run ledger preview isolated so audit-table markup does not bloat the route shell.
 */
export function DashboardRecentRunsPanel({
  dashboard,
  language,
  runSourceSummary,
  t,
}: DashboardRecentRunsPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{t('dashboard.recentRuns')}</span>
        <Link className="panel-action" to="/audit">
          {t('dashboard.fullLedger')}
        </Link>
      </div>
      <div className="panel-body" style={{ padding: 0 }}>
        <table className="data-table">
          <thead>
            <tr>
              <th>{t('dashboard.run')}</th>
              <th>{t('dashboard.type')}</th>
              <th>{t('dashboard.source')}</th>
              <th>{t('dashboard.records')}</th>
              <th>{t('dashboard.status')}</th>
              <th>{t('dashboard.time')}</th>
            </tr>
          </thead>
          <tbody>
            {dashboard.recentRuns.map((run) => (
              <tr key={run.id}>
                <td>
                  <Link
                    className="table-link mono accent"
                    to={`/audit?run=${run.id}`}
                  >
                    #{run.id}
                  </Link>
                </td>
                <td>
                  <span className="tag tag-sm tag-backup">
                    {t(runTypeKey(run.runType ?? 'backup'))}
                  </span>
                </td>
                <td>{runSourceSummary(run.profileScope)}</td>
                <td className="accent">+{run.newVisits}</td>
                <td>
                  <span
                    aria-label={t(runStatusKey(run.status))}
                    className={`status-badge ${
                      run.status === 'success'
                        ? 'status-completed'
                        : 'status-pending'
                    }`}
                  >
                    {t(runStatusKey(run.status))}
                  </span>
                </td>
                <td className="dim">
                  {formatRelativeTime(
                    run.finishedAt ?? run.startedAt,
                    language,
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

interface DashboardArchiveBoundaryPanelProps {
  commonT: Translate
  selectedProfiles: BrowserProfile[]
  t: Translate
}

/**
 * Renders the browser/profile boundary list so retention and missing-history copy stays in one owner.
 */
export function DashboardArchiveBoundaryPanel({
  commonT,
  selectedProfiles,
  t,
}: DashboardArchiveBoundaryPanelProps) {
  function renderProfileBoundary(profile: BrowserProfile) {
    const retention = browserRetentionMeta(profile, commonT)

    return (
      <div key={profile.profileId} className="otd-item">
        <div className="browser-icon">
          <BrowserIcon browserName={profile.browserName} />
        </div>
        <div style={{ minWidth: 0 }}>
          <div className="otd-title">
            {profile.browserName} / {profile.profileName}
          </div>
          <div className="otd-meta mono">
            {profile.historyExists
              ? t('dashboard.historyDetected')
              : t('dashboard.historyMissing')}
          </div>
          {profile.historyExists ? (
            <>
              <div className="otd-meta">{retention.label}</div>
              <div className="mono-support">
                {retention.body} {commonT('browserRetentionArchiveBoundary')}
              </div>
            </>
          ) : null}
        </div>
      </div>
    )
  }

  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{t('dashboard.archiveBoundary')}</span>
        <span className="panel-action">
          {t('dashboard.selectedProfiles', {
            count: selectedProfiles.length,
          })}
        </span>
      </div>
      <div className="panel-body">
        {selectedProfiles.length > 0 ? (
          selectedProfiles.map(renderProfileBoundary)
        ) : (
          <p className="dashboard-next-action">
            {t('dashboard.zeroStateNoBrowsers')}
          </p>
        )}
      </div>
    </div>
  )
}

interface DashboardStorageFootprintPanelProps {
  language: ResolvedLanguage
  storageSegments: DashboardStorageSegment[]
  totalStorage: number
  t: Translate
}

/**
 * Renders the two-band storage footprint panel from the precomputed storage segment view model.
 */
export function DashboardStorageFootprintPanel({
  language,
  storageSegments,
  totalStorage,
  t,
}: DashboardStorageFootprintPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{t('dashboard.storageFootprint')}</span>
        <span className="panel-action">
          {t('dashboard.storageTotal', {
            size: formatBytes(totalStorage, language),
          })}
        </span>
      </div>
      <div className="panel-body">
        <div className="storage-chart">
          {storageSegments.map((segment) => (
            <div key={segment.label} className="storage-row">
              <div className="row-between">
                <span>{segment.label}</span>
                <span className="mono">
                  {formatBytes(segment.value, language)}
                </span>
              </div>
              <div className="storage-bar">
                <div
                  className={segment.tone}
                  style={{
                    width: `${totalStorage > 0 ? (segment.value / totalStorage) * 100 : 0}%`,
                  }}
                />
              </div>
              <div className="mono-support">{segment.detail}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

interface DashboardRhythmPanelProps {
  activeProfileId: string | null
  intelligenceT: Translate
  language: ResolvedLanguage
  refreshToken: number
  t: Translate
}

/**
 * Keeps the dashboard-owned browsing-rhythm entry panel thin while preserving the shared year-pager contract.
 */
export function DashboardRhythmPanel({
  activeProfileId,
  intelligenceT,
  language,
  refreshToken,
  t,
}: DashboardRhythmPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{intelligenceT('rhythmTitle')}</span>
        <Link className="panel-action" to="/intelligence">
          {t('dashboard.reviewInsightsAction')}
        </Link>
      </div>
      <div className="panel-body">
        <BrowsingRhythmCard
          dayDomainHref={(domain, date) =>
            domainDayInsightsHref(domain, date, activeProfileId)
          }
          dayHref={(date) => dayInsightsHref(date, activeProfileId)}
          mode="year"
          language={language}
          profileId={activeProfileId}
          refreshToken={refreshToken}
          showCurrentYearShortcut
          summaryPreset="calendar-year"
          t={intelligenceT}
          yearNavigation="pager"
        />
      </div>
    </div>
  )
}

interface DashboardIntelligencePanelProps {
  aiMeta: ReturnType<typeof aiStatusMeta>
  backgroundQueueCount: number | null
  embeddingProviderId: string | null | undefined
  llmProviderId: string | null | undefined
  language: ResolvedLanguage
  t: Translate
}

/**
 * Renders the dashboard's high-level AI/runtime status card without forcing the route shell to own presentation details.
 */
export function DashboardIntelligencePanel({
  aiMeta,
  backgroundQueueCount,
  embeddingProviderId,
  llmProviderId,
  language,
  t,
}: DashboardIntelligencePanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{t('dashboard.intelligenceTitle')}</span>
        <span className="panel-action">{aiMeta.label}</span>
      </div>
      <div className="panel-body intelligence-stack">
        <p className="dashboard-next-action">{aiMeta.description}</p>
        <div className="summary-stats">
          <div className="summary-stat">
            <span className="dim">{t('dashboard.llmLabel')}</span>
            <span className="mono">
              {llmProviderId ?? t('settings.disabled')}
            </span>
          </div>
          <div className="summary-stat">
            <span className="dim">{t('dashboard.embeddingLabel')}</span>
            <span className="mono">
              {embeddingProviderId ?? t('dashboard.embeddingFallback')}
            </span>
          </div>
          <div className="summary-stat">
            <span className="dim">{t('dashboard.queueLabel')}</span>
            <span className="mono">
              {backgroundQueueCount === null
                ? '—'
                : backgroundQueueCount.toLocaleString(language)}
            </span>
          </div>
        </div>
        <div className="quick-actions-grid">
          <Link className="btn-secondary" to="/explorer?mode=hybrid">
            {t('dashboard.semanticSearchAction')}
          </Link>
          <Link className="btn-secondary" to="/assistant">
            {t('dashboard.openAssistantAction')}
          </Link>
          <Link className="btn-secondary" to="/intelligence">
            {t('dashboard.reviewInsightsAction')}
          </Link>
        </div>
      </div>
    </div>
  )
}

interface DashboardOnThisDayPanelProps {
  activeOnThisDay: OnThisDayEntry[]
  activeOnThisDayError: string | null
  activeProfileId: string | null
  intelligenceT: Translate
  onThisDayLoading: boolean
}

/**
 * Owns the dashboard's compact On This Day preview so day-route deep links stay stable without bloating the route shell.
 */
export function DashboardOnThisDayPanel({
  activeOnThisDay,
  activeOnThisDayError,
  activeProfileId,
  intelligenceT,
  onThisDayLoading,
}: DashboardOnThisDayPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{intelligenceT('onThisDayTitle')}</span>
      </div>
      <div className="panel-body">
        {onThisDayLoading ? (
          <div className="intelligence-stack" aria-busy="true">
            <Skeleton variant="block" height="68px" count={3} />
          </div>
        ) : activeOnThisDay.length > 0 ? (
          activeOnThisDay.slice(0, 3).map((entry) => (
            <div key={`${entry.year}-${entry.date}`} className="otd-item">
              <div style={{ minWidth: 0 }}>
                <div className="otd-title">
                  <Link to={dayInsightsHref(entry.date, activeProfileId)}>
                    {entry.year} ·{' '}
                    {intelligenceT('onThisDayVisits', {
                      count: entry.totalVisits,
                    })}
                  </Link>
                </div>
                {entry.summary ? (
                  <div className="otd-url">{entry.summary}</div>
                ) : null}
                {entry.topDomains.length > 0 ? (
                  <div className="mono-support">
                    {entry.topDomains.slice(0, 4).map((domain, index) => (
                      <span key={`${entry.year}:${domain}`}>
                        {index > 0 ? ' · ' : null}
                        <Link
                          to={domainDayInsightsHref(
                            domain,
                            entry.date,
                            activeProfileId,
                          )}
                        >
                          {domain}
                        </Link>
                      </span>
                    ))}
                  </div>
                ) : null}
              </div>
            </div>
          ))
        ) : (
          <p className="dashboard-next-action">
            {activeOnThisDayError ?? intelligenceT('onThisDayEmpty')}
          </p>
        )}
      </div>
    </div>
  )
}

interface DashboardTrustActionsPanelProps {
  t: Translate
}

/**
 * Keeps dashboard trust-action links grouped together instead of scattering them through the route shell.
 */
export function DashboardTrustActionsPanel({
  t,
}: DashboardTrustActionsPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{t('dashboard.trustActions')}</span>
      </div>
      <div className="panel-body">
        <p className="dashboard-next-action">
          {t('dashboard.trustActionsBody')}
        </p>
        <div className="quick-actions-grid">
          <Link className="btn-secondary" to="/import">
            {t('dashboard.reviewImportBatches')}
          </Link>
          <Link className="btn-secondary" to="/security">
            {t('dashboard.reviewSecurity')}
          </Link>
          <Link className="btn-secondary" to="/schedule">
            {t('dashboard.reviewSchedule')}
          </Link>
        </div>
      </div>
    </div>
  )
}

interface DashboardZeroStateChecklistPanelProps {
  dashboard: DashboardSnapshot
  snapshotInitialized: boolean
  t: Translate
}

/**
 * Renders the dashboard's onboarding checklist card for the archive-zero-state branch.
 */
export function DashboardZeroStateChecklistPanel({
  dashboard,
  snapshotInitialized,
  t,
}: DashboardZeroStateChecklistPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{t('dashboard.zeroStateChecklist')}</span>
      </div>
      <div className="panel-body">
        <div className="stacked-column">
          <div className="list-item">
            <span className={snapshotInitialized ? 'accent' : 'dim'}>
              {snapshotInitialized ? '✓' : '1'}
            </span>
            <span>{t('dashboard.zeroStep1')}</span>
          </div>
          <div className="list-item">
            <span
              className={dashboard.recentRuns.length > 0 ? 'accent' : 'dim'}
            >
              {dashboard.recentRuns.length > 0 ? '✓' : '2'}
            </span>
            <span>{t('dashboard.zeroStep2')}</span>
          </div>
          <div className="list-item">
            <span className="dim">3</span>
            <span>{t('dashboard.zeroStep3')}</span>
          </div>
        </div>
      </div>
    </div>
  )
}
