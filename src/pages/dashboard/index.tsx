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
import { useI18n } from '../../lib/i18n'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import type { OnThisDayEntry } from '../../lib/core-intelligence/types'
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
  DashboardOnThisDayPanel,
  DashboardRecentRunsPanel,
  DashboardRhythmPanel,
  DashboardStatsRow,
  DashboardStorageFootprintPanel,
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

  const readySnapshot = snapshot!
  const readyDashboard = dashboard!

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
          <p className="dashboard-scope-line">
            <span className="mono-kicker">{t('common.profileScope')}</span>
            <span>{activeScopeLabel}</span>
            <span className="dim">· {t('dashboard.scopeNotice')}</span>
          </p>
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

  const nextActionMessage = localizedDashboardNextAction(
    readyDashboard.nextAction,
    t,
  )

  const hasInlineNotices =
    needsKeyringReview || safariNeedsAccess || Boolean(nextActionMessage)

  return (
    <section className="page-shell" data-testid="dashboard-page">
      {hasInlineNotices ? (
        <div className="dashboard-notice-stack">
          {nextActionMessage ? (
            <div className="warning-box warning-box--info">
              <div className="warning-icon">{'>'}</div>
              <div className="warning-text">
                <strong>{t('dashboard.nextActionEyebrow')}</strong>{' '}
                <span>{nextActionMessage}</span>
              </div>
            </div>
          ) : null}
          {needsKeyringReview ? (
            <div className="warning-box">
              <div className="warning-icon">{'!'}</div>
              <div className="warning-text">
                <strong>{t('platform.keyringTitle')}</strong>{' '}
                <span>{t('platform.keyringBody')}</span>{' '}
                <Link className="warning-box__action" to="/security">
                  {t('dashboard.reviewSecurity')} →
                </Link>
              </div>
            </div>
          ) : null}
          {safariNeedsAccess ? (
            <div className="warning-box warning-box--danger">
              <div className="warning-icon">{'!'}</div>
              <div className="warning-text">
                <strong>{t('platform.safariAccessTitle')}</strong>{' '}
                <span>{t('platform.safariAccessBody')}</span>{' '}
                <Link className="warning-box__action" to="/import">
                  {t('dashboard.reviewImportBatches')} →
                </Link>
              </div>
            </div>
          ) : null}
        </div>
      ) : null}

      {activeProfileId ? (
        <p className="dashboard-scope-line">
          <span className="mono-kicker">{t('common.profileScope')}</span>
          <span>{activeScopeLabel}</span>
          <span className="dim">· {t('dashboard.scopeNotice')}</span>
        </p>
      ) : null}

      <DashboardStatsRow stats={stats} />

      <div className="dashboard-grid">
        <div className="dashboard-left">
          <DashboardRecentRunsPanel
            dashboard={readyDashboard}
            language={language}
            runSourceSummary={runSourceSummary}
            t={t}
          />

          <DashboardRhythmPanel
            activeProfileId={activeProfileId}
            intelligenceT={intelligenceT}
            language={language}
            refreshToken={refreshKey}
            t={t}
          />
        </div>

        <div className="dashboard-right">
          <DashboardOnThisDayPanel
            activeOnThisDay={activeOnThisDay}
            activeOnThisDayError={activeOnThisDayError}
            activeProfileId={activeProfileId}
            intelligenceT={intelligenceT}
            onThisDayLoading={onThisDayLoading}
          />

          <DashboardStorageFootprintPanel
            language={language}
            storageSegments={storageSegments}
            totalStorage={totalStorage}
            t={t}
          />
        </div>
      </div>
    </section>
  )
}

function localizedDashboardNextAction(
  nextAction: string | null | undefined,
  t: (key: string, vars?: Record<string, string | number>) => string,
) {
  const normalized = nextAction?.trim()
  if (!normalized) {
    return null
  }

  if (normalized.includes('Initialize the archive')) {
    return t('dashboard.nextActionInitializeArchive')
  }

  if (
    normalized.includes(
      'Run a manual backup to create the first manifest and snapshot artifacts',
    )
  ) {
    return t('dashboard.nextActionRunFirstBackup')
  }

  return normalized
}
