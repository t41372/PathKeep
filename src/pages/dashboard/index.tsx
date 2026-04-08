import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { DashboardSkeleton } from '../../components/primitives/skeleton'
import { backend } from '../../lib/backend'
import {
  calendarDayKey,
  formatBytes,
  formatDateTime,
  formatRelativeTime,
} from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  aiStatusMeta,
  dedupeEvidence,
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
import type { InsightSnapshot } from '../../lib/types'

function isBackupReadyProfile(profile: {
  profileId: string
  historyExists: boolean
}) {
  return profile.historyExists
}

function browserIconClass(profileId: string) {
  if (profileId.startsWith('chrome:')) return 'chrome'
  if (profileId.startsWith('arc:')) return 'arc'
  if (profileId.startsWith('firefox:')) return 'firefox'
  if (profileId.startsWith('safari:')) return 'safari'
  return ''
}

function browserIconLetter(profileId: string) {
  if (profileId.startsWith('chrome:')) return 'C'
  if (profileId.startsWith('arc:')) return 'A'
  if (profileId.startsWith('firefox:')) return 'F'
  if (profileId.startsWith('safari:')) return 'S'
  return '?'
}

function flattenInsightEvidence(snapshot: InsightSnapshot) {
  return dedupeEvidence([
    ...snapshot.cards.flatMap((card) => card.evidence),
    ...snapshot.topics.flatMap((topic) => topic.evidence),
    ...snapshot.threads.flatMap((thread) => thread.evidence),
  ])
}

export function DashboardPage() {
  const { dashboard, error, loading, refreshKey, snapshot } = useShellData()
  const { language, t, ns } = useI18n()
  const { activeProfileId } = useProfileScope()
  const insightsT = ns('insights')
  const intelligenceT = ns('intelligence')
  const [insights, setInsights] = useState<InsightSnapshot | null>(null)
  const [insightLoadError, setInsightLoadError] = useState<string | null>(null)

  useEffect(() => {
    if (!snapshot?.config.initialized) {
      return
    }

    let cancelled = false

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
        <DashboardSkeleton />
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
  const allInsightEvidence = activeInsights
    ? flattenInsightEvidence(activeInsights)
    : []
  const todayKey = calendarDayKey(new Date())
  const onThisDay = allInsightEvidence
    .filter((item) => calendarDayKey(item.visitedAt) === todayKey)
    .slice(0, 3)
  const periodicSummary = (() => {
    if (!activeInsights) return []
    const seeded = activeInsights.cards.slice(0, 2).map((card) => card.summary)
    return seeded.length > 0 ? seeded : activeInsights.notes
  })()

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
        <EmptyState
          action={
            <Link className="btn-primary" to="/onboarding">
              {t('dashboard.openOnboardingFlow')}
            </Link>
          }
          description={dashboard.nextAction ?? t('dashboard.zeroStateBody')}
          eyebrow={t('dashboard.zeroStateEyebrow')}
          title={t('dashboard.zeroStateTitle')}
        />
      </section>
    )
  }

  const needsKeyringReview =
    snapshot.config.archiveMode === 'Encrypted' &&
    snapshot.config.rememberDatabaseKeyInKeyring &&
    !snapshot.keyringStatus.storedSecret
  const safariNeedsAccess = hasSafariAccessIssue(selectedProfiles)

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
              {selectedProfiles.map((profile) => (
                <div key={profile.profileId} className="otd-item">
                  <div
                    className={`browser-icon ${browserIconClass(profile.profileId)}`}
                  >
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
                  </div>
                </div>
              ))}
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
              {onThisDay.length > 0 ? (
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
              {periodicSummary.length > 0 ? (
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
      </div>
    </section>
  )
}
