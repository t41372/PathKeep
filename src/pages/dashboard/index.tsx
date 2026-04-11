/**
 * This module renders the Dashboard route, which summarizes archive health, recent runs, scoped callouts, and quick links into the rest of the app.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `DashboardPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { ErrorState } from '../../components/primitives/error-state'
import {
  DashboardSkeleton,
  Skeleton,
} from '../../components/primitives/skeleton'
import { backend } from '../../lib/backend-client'
import { browserRetentionMeta } from '../../lib/browser-retention'
import {
  calendarDayKey,
  formatBytes,
  formatDateTime,
  formatRelativeTime,
} from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  resolveInsightOnThisDay,
  resolveInsightPeriodicSummary,
} from '../../lib/insight-canonical'
import {
  aiStatusMeta,
  evidenceHref,
  selectedAiProvider,
} from '../../lib/intelligence'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import { hasSafariAccessIssue } from '../../lib/platform-guidance'
import {
  archiveModeKey,
  runStatusKey,
  runTypeKey,
  sourceKindFromProfileScope,
} from '../../lib/trust-review'
import type { BrowserProfile, InsightSnapshot } from '../../lib/types'

/**
 * Returns whether backup ready profile.
 *
 * Keeping this as a named declaration makes the Dashboard surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function isBackupReadyProfile(profile: {
  profileId: string
  historyExists: boolean
}) {
  return profile.historyExists
}

/**
 * Explains how browser icon class works.
 *
 * Keeping this as a named declaration makes the Dashboard surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function browserIconClass(profileId: string) {
  if (profileId.startsWith('chrome:')) return 'chrome'
  if (profileId.startsWith('arc:')) return 'arc'
  if (profileId.startsWith('firefox:')) return 'firefox'
  if (profileId.startsWith('safari:')) return 'safari'
  return ''
}

/**
 * Explains how browser icon letter works.
 *
 * Keeping this as a named declaration makes the Dashboard surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function browserIconLetter(profileId: string) {
  if (profileId.startsWith('chrome:')) return 'C'
  if (profileId.startsWith('arc:')) return 'A'
  if (profileId.startsWith('firefox:')) return 'F'
  if (profileId.startsWith('safari:')) return 'S'
  return '?'
}

/**
 * Renders the dashboard route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Dashboard expectations in the design docs.
 */
export function DashboardPage() {
  const { dashboard, error, loading, refreshKey, snapshot } = useShellData()
  const { language, t, ns } = useI18n()
  const { activeProfileId } = useProfileScope()
  const commonT = ns('common')
  const insightsT = ns('insights')
  const intelligenceT = ns('intelligence')
  const [insights, setInsights] = useState<InsightSnapshot | null>(null)
  const [insightsLoading, setInsightsLoading] = useState(false)
  const [insightLoadError, setInsightLoadError] = useState<string | null>(null)

  useEffect(() => {
    if (!snapshot?.config.initialized) {
      setInsightsLoading(false)
      return
    }

    let cancelled = false
    setInsightsLoading(true)

    /**
     * Explains how load works.
     *
     * Keeping this as a named declaration makes the Dashboard surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const load = async () => {
      try {
        const nextInsights = await backend.loadInsights({
          fullRebuild: false,
          profileId: activeProfileId,
        })
        if (!cancelled) {
          setInsights(nextInsights)
          setInsightLoadError(null)
        }
      } catch (nextError) {
        if (!cancelled) {
          setInsights(null)
          setInsightLoadError(
            nextError instanceof Error
              ? nextError.message
              : insightsT('refreshAttentionTitle'),
          )
        }
      } finally {
        if (!cancelled) {
          setInsightsLoading(false)
        }
      }
    }

    void load()

    return () => {
      cancelled = true
    }
  }, [activeProfileId, insightsT, refreshKey, snapshot?.config.initialized])

  if (loading && !dashboard) {
    return (
      <section className="page-shell" data-testid="dashboard-page">
        <DashboardSkeleton label={t('common.loadingDashboard')} />
      </section>
    )
  }

  if (error && !dashboard) {
    return (
      <section className="page-shell" data-testid="dashboard-page">
        <ErrorState
          title={t('dashboard.archiveReadError')}
          description={error}
        />
      </section>
    )
  }

  if (!snapshot || !dashboard) {
    return (
      <section className="page-shell" data-testid="dashboard-page">
        <ErrorState
          title={t('dashboard.archiveUnavailable')}
          description={t('dashboard.archiveUnavailableBody')}
        />
      </section>
    )
  }

  const selectedProfiles = snapshot.browserProfiles.filter((profile) =>
    snapshot.config.selectedProfileIds.includes(profile.profileId),
  )
  const activeScopeLabel = activeProfileId
    ? profileIdLabel(activeProfileId)
    : t('common.profileAllProfiles')
  const backupReadyProfiles = selectedProfiles.filter((profile) =>
    isBackupReadyProfile(profile),
  )
  const previewOnlyProfiles = selectedProfiles.filter(
    (profile) => !isBackupReadyProfile(profile),
  )
  const totalStorage =
    dashboard.storage.archiveDatabaseBytes +
    dashboard.storage.manifestBytes +
    dashboard.storage.snapshotBytes +
    dashboard.storage.exportBytes
  const latestManifestHash =
    dashboard.recentRuns.find((run) => run.manifestHash)?.manifestHash ?? null
  const aiMeta = aiStatusMeta(snapshot.aiStatus, intelligenceT)
  const llmProvider = selectedAiProvider(snapshot.config.ai, 'llm')
  const embeddingProvider = selectedAiProvider(snapshot.config.ai, 'embedding')
  const activeInsights = snapshot.config.initialized ? insights : null
  const activeInsightLoadError = snapshot.config.initialized
    ? insightLoadError
    : null
  const todayKey = calendarDayKey(new Date())
  const onThisDay = activeInsights
    ? resolveInsightOnThisDay(activeInsights, todayKey, 3)
    : []
  const periodicSummary = activeInsights
    ? resolveInsightPeriodicSummary(activeInsights, insightsT)
    : []

  /**
   * Explains how run source summary works.
   *
   * Keeping this as a named declaration makes the Dashboard surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function runSourceSummary(profileScope: string[] | undefined) {
    const sourceKinds = sourceKindFromProfileScope(profileScope ?? [])
    return sourceKinds
      .map((sourceKind) => {
        if (sourceKind === 'chrome') return t('audit.sourceChrome')
        if (sourceKind === 'firefox') return t('audit.sourceFirefox')
        if (sourceKind === 'safari') return t('audit.sourceSafari')
        if (sourceKind === 'takeout') return t('audit.sourceTakeout')
        if (sourceKind === 'archive-wide') return t('audit.archiveWide')
        return sourceKind
      })
      .join(' · ')
  }

  /**
   * Explains how render profile boundary works.
   *
   * Keeping this as a named declaration makes the Dashboard surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function renderProfileBoundary(profile: BrowserProfile) {
    const retention = browserRetentionMeta(profile, commonT)

    return (
      <div key={profile.profileId} className="otd-item">
        <div className={`browser-icon ${browserIconClass(profile.profileId)}`}>
          {browserIconLetter(profile.profileId)}
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

  const storageSegments = [
    {
      label: t('dashboard.archiveDatabase'),
      tone: 'storage-fill',
      value: dashboard.storage.archiveDatabaseBytes,
    },
    {
      label: t('dashboard.manifests'),
      tone: 'storage-fill secondary',
      value: dashboard.storage.manifestBytes,
    },
    {
      label: t('dashboard.snapshots'),
      tone: 'storage-fill tertiary',
      value: dashboard.storage.snapshotBytes,
    },
    {
      label: t('dashboard.exports'),
      tone: 'storage-fill dim',
      value: dashboard.storage.exportBytes,
    },
  ]
  const stats = [
    {
      label: t('dashboard.totalRecords'),
      value: dashboard.totalVisits.toLocaleString(language),
      detail: t('dashboard.uniqueUrls', {
        count: dashboard.totalUrls.toLocaleString(language),
      }),
      tone: 'accent' as const,
    },
    {
      label: t('dashboard.lastBackup'),
      value: dashboard.lastSuccessfulBackupAt
        ? formatRelativeTime(dashboard.lastSuccessfulBackupAt, language)
        : t('common.pending'),
      detail: latestManifestHash ?? t('dashboard.noManifestYet'),
      tone: dashboard.lastSuccessfulBackupAt
        ? ('success' as const)
        : ('neutral' as const),
    },
    {
      label: t('dashboard.profilesInScope'),
      value: `${selectedProfiles.length}`,
      detail: t('dashboard.profilesReadableAttention', {
        readable: backupReadyProfiles.length,
        attention: previewOnlyProfiles.length,
      }),
      tone: 'neutral' as const,
    },
    {
      label: t('dashboard.archiveMode'),
      value: t(archiveModeKey(snapshot.config.archiveMode)),
      detail: snapshot.archiveStatus.unlocked
        ? t('dashboard.archiveUnlocked')
        : t('dashboard.archiveNeedsUnlock'),
      tone: snapshot.archiveStatus.unlocked
        ? ('success' as const)
        : ('neutral' as const),
    },
  ]

  if (!snapshot.config.initialized || dashboard.recentRuns.length === 0) {
    return (
      <section className="page-shell" data-testid="dashboard-page">
        {activeProfileId ? (
          <StatusCallout
            tone="info"
            eyebrow={t('common.profileScope')}
            title={activeScopeLabel}
            body={t('dashboard.scopeNotice')}
          />
        ) : null}
        <div className="stats-row">
          {stats.map((stat) => (
            <article
              key={stat.label}
              className="stat-card"
              data-tone={stat.tone}
            >
              <div className="stat-label">{stat.label}</div>
              <div className="stat-value">{stat.value}</div>
              <div className="stat-delta neutral">{stat.detail}</div>
            </article>
          ))}
        </div>
        <StatusCallout
          tone="info"
          eyebrow={t('dashboard.zeroStateEyebrow')}
          title={t('dashboard.zeroStateTitle')}
          body={dashboard.nextAction ?? t('dashboard.zeroStateBody')}
          actions={
            <Link className="btn-primary" to="/onboarding">
              {t('dashboard.openOnboardingFlow')}
            </Link>
          }
        />
        <div className="dashboard-grid">
          <div className="dashboard-left">
            <div className="panel">
              <div className="panel-header">
                <span className="panel-title">
                  {t('dashboard.archiveBoundary')}
                </span>
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
          </div>
          <div className="dashboard-right">
            <div className="panel">
              <div className="panel-header">
                <span className="panel-title">
                  {t('dashboard.zeroStateChecklist')}
                </span>
              </div>
              <div className="panel-body">
                <div className="stacked-column">
                  <div className="list-item">
                    <span
                      className={snapshot.config.initialized ? 'accent' : 'dim'}
                    >
                      {snapshot.config.initialized ? '✓' : '1'}
                    </span>
                    <span>{t('dashboard.zeroStep1')}</span>
                  </div>
                  <div className="list-item">
                    <span
                      className={
                        dashboard.recentRuns.length > 0 ? 'accent' : 'dim'
                      }
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
          </div>
        </div>
      </section>
    )
  }

  const needsKeyringReview =
    snapshot.config.archiveMode === 'Encrypted' &&
    snapshot.config.rememberDatabaseKeyInKeyring &&
    !snapshot.keyringStatus.storedSecret
  const safariNeedsAccess = hasSafariAccessIssue(selectedProfiles)

  const nextActionMessage = dashboard.nextAction ?? null

  return (
    <section className="page-shell" data-testid="dashboard-page">
      {activeProfileId ? (
        <StatusCallout
          tone="info"
          eyebrow={t('common.profileScope')}
          title={activeScopeLabel}
          body={t('dashboard.scopeNotice')}
        />
      ) : null}

      {nextActionMessage ? (
        <StatusCallout
          tone="info"
          eyebrow={t('dashboard.nextActionEyebrow')}
          title={nextActionMessage}
        />
      ) : null}

      {(needsKeyringReview || safariNeedsAccess) && (
        <div className="dashboard-callouts">
          {needsKeyringReview ? (
            <StatusCallout
              tone="warning"
              title={t('platform.keyringTitle')}
              body={t('platform.keyringBody')}
              actions={
                <Link className="btn-secondary" to="/security">
                  {t('dashboard.reviewSecurity')}
                </Link>
              }
            />
          ) : null}
          {safariNeedsAccess ? (
            <StatusCallout
              tone="blocked"
              title={t('platform.safariAccessTitle')}
              body={t('platform.safariAccessBody')}
              actions={
                <Link className="btn-secondary" to="/import">
                  {t('dashboard.reviewImportBatches')}
                </Link>
              }
            />
          ) : null}
        </div>
      )}

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

      <div className="dashboard-grid">
        <div className="dashboard-left">
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

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">
                {t('dashboard.archiveBoundary')}
              </span>
              <span className="panel-action">
                {t('dashboard.selectedProfiles', {
                  count: selectedProfiles.length,
                })}
              </span>
            </div>
            <div className="panel-body">
              {selectedProfiles.map(renderProfileBoundary)}
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">
                {t('dashboard.storageFootprint')}
              </span>
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
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>

        <div className="dashboard-right">
          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">
                {t('dashboard.intelligenceTitle')}
              </span>
              <span className="panel-action">{aiMeta.label}</span>
            </div>
            <div className="panel-body intelligence-stack">
              <p className="dashboard-next-action">{aiMeta.description}</p>
              <div className="summary-stats">
                <div className="summary-stat">
                  <span className="dim">{t('dashboard.llmLabel')}</span>
                  <span className="mono">
                    {llmProvider?.id ?? t('settings.disabled')}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{t('dashboard.embeddingLabel')}</span>
                  <span className="mono">
                    {embeddingProvider?.id ?? t('dashboard.embeddingFallback')}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{t('dashboard.queueLabel')}</span>
                  <span className="mono">
                    {snapshot.aiStatus.queuedJobs +
                      snapshot.aiStatus.runningJobs}
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
                <Link className="btn-secondary" to="/insights">
                  {t('dashboard.reviewInsightsAction')}
                </Link>
              </div>
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">{insightsT('onThisDay')}</span>
              <Link className="panel-action" to="/insights">
                {t('dashboard.reviewInsightsAction')}
              </Link>
            </div>
            <div className="panel-body">
              {insightsLoading ? (
                <div className="intelligence-stack" aria-busy="true">
                  <Skeleton variant="block" height="68px" count={3} />
                </div>
              ) : onThisDay.length > 0 ? (
                onThisDay.map((item) => (
                  <Link
                    key={`${item.historyId}-${item.url}`}
                    className="otd-item dashboard-evidence-link"
                    to={evidenceHref(item)}
                  >
                    <div style={{ minWidth: 0 }}>
                      <div className="otd-title">{item.title ?? item.url}</div>
                      <div className="otd-url">{item.url}</div>
                      <div className="mono-support">
                        {formatDateTime(item.visitedAt, language) ??
                          item.visitedAt}
                      </div>
                    </div>
                  </Link>
                ))
              ) : (
                <p className="dashboard-next-action">
                  {activeInsightLoadError ??
                    insightsT('nothingForDayDescription')}
                </p>
              )}
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">
                {insightsT('periodicSummary')}
              </span>
              <span className="panel-action">
                {activeInsights
                  ? insightsT('snapshotLabel', {
                      time: formatRelativeTime(
                        activeInsights.generatedAt,
                        language,
                      ),
                    })
                  : t('common.pending')}
              </span>
            </div>
            <div className="panel-body">
              {insightsLoading ? (
                <div className="intelligence-stack" aria-busy="true">
                  <Skeleton variant="block" height="78px" count={2} />
                </div>
              ) : periodicSummary.length > 0 ? (
                <div className="otd-summary">
                  {periodicSummary.slice(0, 2).map((paragraph) => (
                    <p key={paragraph}>{paragraph}</p>
                  ))}
                </div>
              ) : (
                <p className="dashboard-next-action">
                  {activeInsightLoadError ?? aiMeta.description}
                </p>
              )}
            </div>
          </div>

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
        </div>
      </div>
    </section>
  )
}
