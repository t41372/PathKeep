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
import { useI18n } from '../../lib/i18n'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import type { OnThisDayEntry } from '../../lib/core-intelligence/types'
import {
  aiStatusMeta,
  selectedAiProvider,
} from '../../lib/intelligence-ai-presentation'
import { buildStorageAnalyticsSummary } from '../../lib/storage-analytics'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import { hasSafariAccessIssue } from '../../lib/platform-guidance'
import {
  buildDashboardStats,
  buildDashboardStorageSegments,
  isBackupReadyProfile,
  summarizeRunSources,
} from './helpers'
import {
  DashboardArchiveBoundaryPanel,
  DashboardIntelligencePanel,
  DashboardOnThisDayPanel,
  DashboardRecentRunsPanel,
  DashboardRhythmPanel,
  DashboardStatsRow,
  DashboardStorageFootprintPanel,
  DashboardTrustActionsPanel,
} from './panels'
import { useDashboardArchiveAccessFallback } from './route-fallback-access'
import { DashboardRouteFallback } from './route-fallback'
import { resolveDashboardRouteFallback } from './route-fallback-state'
import { DashboardZeroState } from './zero-state'

/**
 * Renders the dashboard route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Dashboard expectations in the design docs.
 */
export function DashboardPage() {
  const {
    dashboard,
    dashboardLoading = false,
    error,
    loading,
    refreshKey,
    runtimeStatus = {
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    },
    snapshot,
  } = useShellData()
  const { language, t, ns } = useI18n()
  const { activeProfileId } = useProfileScope()
  const commonT = ns('common')
  const intelligenceT = ns('intelligence')
  const [onThisDayEntries, setOnThisDayEntries] = useState<OnThisDayEntry[]>([])
  const [onThisDayLoading, setOnThisDayLoading] = useState(false)
  const [onThisDayError, setOnThisDayError] = useState<string | null>(null)
  const archiveAccessFallback = useDashboardArchiveAccessFallback({
    dashboard,
    error,
    refreshKey,
    snapshot,
  })
  const backgroundQueueCount =
    runtimeStatus.aiQueue && runtimeStatus.intelligence
      ? runtimeStatus.aiQueue.queued +
        runtimeStatus.aiQueue.running +
        runtimeStatus.aiQueue.failed +
        runtimeStatus.intelligence.queue.queued +
        runtimeStatus.intelligence.queue.running +
        runtimeStatus.intelligence.queue.failed
      : null

  useEffect(() => {
    if (!snapshot?.config.initialized) {
      setOnThisDayLoading(false)
      return
    }

    let cancelled = false
    setOnThisDayLoading(true)

    const load = async () => {
      try {
        const entries = await coreIntelligenceApi.getOnThisDay(activeProfileId)
        if (!cancelled) {
          setOnThisDayEntries(entries.data ?? [])
          setOnThisDayError(null)
        }
      } catch (nextError) {
        if (!cancelled) {
          setOnThisDayEntries([])
          setOnThisDayError(
            nextError instanceof Error
              ? nextError.message
              : intelligenceT('onThisDayEmpty'),
          )
        }
      } finally {
        if (!cancelled) {
          setOnThisDayLoading(false)
        }
      }
    }

    void load()

    return () => {
      cancelled = true
    }
  }, [activeProfileId, intelligenceT, refreshKey, snapshot?.config.initialized])

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

  if (!snapshot || !dashboard) {
    return (
      <DashboardRouteFallback state={{ kind: 'archive-unavailable' }} t={t} />
    )
  }

  const readySnapshot = snapshot
  const readyDashboard = dashboard

  const selectedProfiles = readySnapshot.browserProfiles.filter((profile) =>
    readySnapshot.config.selectedProfileIds.includes(profile.profileId),
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
  const storageSummary = buildStorageAnalyticsSummary(readyDashboard.storage)
  const totalStorage = storageSummary.trackedStorageBytes
  const latestManifestHash =
    readyDashboard.recentRuns.find((run) => run.manifestHash)?.manifestHash ??
    null
  const aiMeta = aiStatusMeta(readySnapshot.aiStatus, intelligenceT)
  const llmProvider = selectedAiProvider(readySnapshot.config.ai, 'llm')
  const embeddingProvider = selectedAiProvider(
    readySnapshot.config.ai,
    'embedding',
  )
  const activeOnThisDay = readySnapshot.config.initialized
    ? onThisDayEntries
    : []
  const activeOnThisDayError = readySnapshot.config.initialized
    ? onThisDayError
    : null
  const storageSegments = buildDashboardStorageSegments(commonT, storageSummary)
  const stats = buildDashboardStats({
    dashboard: readyDashboard,
    snapshot: readySnapshot,
    selectedProfilesCount: selectedProfiles.length,
    backupReadyProfilesCount: backupReadyProfiles.length,
    previewOnlyProfilesCount: previewOnlyProfiles.length,
    language,
    latestManifestHash,
    t,
  })
  const runSourceSummary = (profileScope: string[] | undefined) =>
    summarizeRunSources(profileScope, t)

  if (
    !readySnapshot.config.initialized ||
    readyDashboard.recentRuns.length === 0
  ) {
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
        <DashboardZeroState
          commonT={commonT}
          dashboard={readyDashboard}
          selectedProfiles={selectedProfiles}
          snapshotInitialized={readySnapshot.config.initialized}
          stats={stats}
          t={t}
        />
      </section>
    )
  }

  const needsKeyringReview =
    readySnapshot.config.archiveMode === 'Encrypted' &&
    readySnapshot.config.rememberDatabaseKeyInKeyring &&
    !readySnapshot.keyringStatus.storedSecret
  const safariNeedsAccess = hasSafariAccessIssue(selectedProfiles)

  const nextActionMessage = readyDashboard.nextAction ?? null

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

      <DashboardStatsRow stats={stats} />

      <DashboardRhythmPanel
        activeProfileId={activeProfileId}
        intelligenceT={intelligenceT}
        language={language}
        t={t}
      />

      <div className="dashboard-grid">
        <div className="dashboard-left">
          <DashboardRecentRunsPanel
            dashboard={readyDashboard}
            language={language}
            runSourceSummary={runSourceSummary}
            t={t}
          />

          <DashboardArchiveBoundaryPanel
            commonT={commonT}
            selectedProfiles={selectedProfiles}
            t={t}
          />

          <DashboardStorageFootprintPanel
            language={language}
            storageSegments={storageSegments}
            totalStorage={totalStorage}
            t={t}
          />
        </div>

        <div className="dashboard-right">
          <DashboardIntelligencePanel
            aiMeta={aiMeta}
            backgroundQueueCount={backgroundQueueCount}
            embeddingProviderId={embeddingProvider?.id}
            language={language}
            llmProviderId={llmProvider?.id}
            t={t}
          />

          <DashboardOnThisDayPanel
            activeOnThisDay={activeOnThisDay}
            activeOnThisDayError={activeOnThisDayError}
            activeProfileId={activeProfileId}
            intelligenceT={intelligenceT}
            onThisDayLoading={onThisDayLoading}
          />

          <DashboardTrustActionsPanel t={t} />
        </div>
      </div>
    </section>
  )
}
