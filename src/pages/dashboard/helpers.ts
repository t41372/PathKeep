/**
 * @file helpers.ts
 * @description Holds the pure helper contracts behind the Dashboard route so the route shell can focus on data orchestration and gating.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Keep stats and source-summary derivations deterministic and easy to test.
 * - Define the small view-model shapes reused by split dashboard panels.
 *
 * ## Not responsible for
 * - Fetching dashboard or intelligence data
 * - Rendering dashboard panels or route-level error states
 *
 * ## Dependencies
 * - Depends on archive/dashboard read-model types plus shared formatting helpers.
 *
 * ## Performance notes
 * - Pure helper module only; keeping these derivations allocation-light helps the dashboard stay responsive on large archives.
 */

import { formatRelativeTime } from '../../lib/format'
import type { ResolvedLanguage } from '../../lib/i18n'
import {
  archiveModeKey,
  sourceKindFromProfileScope,
} from '../../lib/trust-review'
import type { DashboardSnapshot, AppSnapshot } from '../../lib/types'
import type { StorageAnalyticsSummary } from '../../lib/storage-analytics'

type Translate = (key: string, vars?: Record<string, string | number>) => string

/**
 * Describes one top-line stat card rendered by the dashboard shell.
 */
export interface DashboardStatItem {
  label: string
  value: string
  detail: string
  tone: 'accent' | 'success' | 'neutral'
}

/**
 * Describes one stacked storage segment row in the dashboard footprint panel.
 */
export interface DashboardStorageSegment {
  label: string
  detail: string
  tone: string
  value: number
}

/**
 * Keeps the dashboard's readable-profile logic explicit instead of burying it inside route filters.
 */
export function isBackupReadyProfile(profile: {
  profileId: string
  historyExists: boolean
}) {
  return profile.historyExists
}

/**
 * Summarizes one backup run's profile scope into the human-readable source strip used by the recent-runs table.
 */
export function summarizeRunSources(
  profileScope: string[] | undefined,
  t: Translate,
) {
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

interface BuildDashboardStatsArgs {
  dashboard: DashboardSnapshot
  snapshot: AppSnapshot
  selectedProfilesCount: number
  backupReadyProfilesCount: number
  previewOnlyProfilesCount: number
  language: ResolvedLanguage
  latestManifestHash: string | null
  t: Translate
}

/**
 * Builds the dashboard's top-line stat cards in one deterministic place so shell and zero-state branches stay aligned.
 */
export function buildDashboardStats({
  dashboard,
  snapshot,
  selectedProfilesCount,
  backupReadyProfilesCount,
  previewOnlyProfilesCount,
  language,
  latestManifestHash,
  t,
}: BuildDashboardStatsArgs): DashboardStatItem[] {
  return [
    {
      label: t('dashboard.totalRecords'),
      value: dashboard.totalVisits.toLocaleString(language),
      detail: t('dashboard.uniqueUrls', {
        count: dashboard.totalUrls.toLocaleString(language),
      }),
      tone: 'accent',
    },
    {
      label: t('dashboard.lastBackup'),
      value: dashboard.lastSuccessfulBackupAt
        ? formatRelativeTime(dashboard.lastSuccessfulBackupAt, language)
        : t('common.pending'),
      detail: latestManifestHash ?? t('dashboard.noManifestYet'),
      tone: dashboard.lastSuccessfulBackupAt ? 'success' : 'neutral',
    },
    {
      label: t('dashboard.profilesInScope'),
      value: `${selectedProfilesCount}`,
      detail: t('dashboard.profilesReadableAttention', {
        readable: backupReadyProfilesCount,
        attention: previewOnlyProfilesCount,
      }),
      tone: 'neutral',
    },
    {
      label: t('dashboard.archiveMode'),
      value: t(archiveModeKey(snapshot.config.archiveMode)),
      detail: snapshot.archiveStatus.unlocked
        ? t('dashboard.archiveUnlocked')
        : t('dashboard.archiveNeedsUnlock'),
      tone: snapshot.archiveStatus.unlocked ? 'success' : 'neutral',
    },
  ]
}

/**
 * Groups storage summary bytes into the two user-facing segments already shipped by the dashboard.
 */
export function buildDashboardStorageSegments(
  commonT: Translate,
  storageSummary: StorageAnalyticsSummary,
): DashboardStorageSegment[] {
  return [
    {
      label: commonT('coreHistory'),
      detail: [commonT('canonicalArchive'), commonT('sourceEvidence')].join(
        ' · ',
      ),
      tone: 'storage-fill',
      value: storageSummary.coreHistoryBytes,
    },
    {
      label: commonT('otherData'),
      detail: [
        commonT('searchProjection'),
        commonT('intelligenceProjection'),
        commonT('semanticIndex'),
        commonT('contentBlobs'),
        commonT('auditArtifacts'),
        commonT('exports'),
        commonT('temporaryFiles'),
      ].join(' · '),
      tone: 'storage-fill secondary',
      value: storageSummary.otherDataBytes,
    },
  ]
}
