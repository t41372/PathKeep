/**
 * @file derived-runtime-review.tsx
 * @description Renders the runtime, module, plugin, and recent-job review surface for Settings derived-state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 runtime queue summary、deterministic modules、enrichment plugins、recent jobs 與 rebuild/clear results。
 * - 把 retry/cancel/toggle 行為交回 route-owned handlers。
 * - 維持 derived-state runtime review 與 Jobs/Audit deep links 的誠實關係。
 *
 * ## 不負責
 * - 不載入 runtime snapshot。
 * - 不管理 search-rule editor。
 * - 不改變 runtime queue grammar。
 *
 * ## 依賴關係
 * - 依賴 route hook 提供 runtime snapshot、dashboard recent run 與 handlers。
 * - 依賴 intelligence runtime helper labels。
 *
 * ## 性能備注
 * - 只根據既有 snapshot/runtime payload 派生小型 display models，不做額外 IO。
 */

import { useMemo } from 'react'
import { Link } from 'react-router-dom'
import { ReviewRuntimeBoundaryCard } from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import {
  READABLE_CONTENT_REFETCH_PLUGIN_ID,
  enrichmentPluginRegistry,
  enrichmentPluginState,
  resolveEnrichmentSettings,
} from '../../lib/enrichment'
import { formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  deterministicModuleDescription,
  deterministicModuleLabel,
  deterministicModuleStatusLabel,
  enrichmentPluginBoundaryLabel,
  enrichmentPluginDescription,
  enrichmentPluginLabel,
} from '../../lib/intelligence-runtime'
import type {
  AppSnapshot,
  ClearDerivedIntelligenceReport,
  DashboardSnapshot,
  DerivedRuntimeNote,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'
import type { CoreIntelligenceQueueReport } from '../../lib/core-intelligence/types'

type Translate = (key: string, vars?: Record<string, string | number>) => string

/**
 * Maps `DERIVED_NOTE_*` codes from `vault-core/src/models/intelligence.rs` onto
 * shipped Settings copy.
 *
 * Keyed by stable code — never by backend prose. The previous version of this
 * module carried a regex table (`^Rebuilt all daily rollups for (.+)\.$` and
 * friends) that silently fell back to English the moment Rust changed a
 * sentence.
 */
const DERIVED_NOTE_KEY_BY_CODE: Record<string, string> = {
  'visit-facts-cleared-no-visits':
    'deterministicModuleNoVisibleVisitsClearedVisitFacts',
  'visit-facts-up-to-date': 'deterministicModuleVisitFactsUpToDate',
  'visit-facts-refreshed': 'deterministicModuleVisitFactsRefreshed',
  'visit-facts-rebuilt': 'deterministicModuleVisitFactsRebuilt',
  'daily-rollups-cleared-no-visits':
    'deterministicModuleNoVisibleVisitsClearedDailyRollups',
  'daily-rollups-up-to-date': 'deterministicModuleDailyRollupsUpToDate',
  'daily-rollups-refreshed': 'deterministicModuleDailyRollupsRefreshed',
  'daily-rollups-rebuilt': 'deterministicModuleDailyRollupsRebuilt',
  'structural-cleared-no-visits':
    'deterministicModuleNoVisibleVisitsClearedStructural',
  'structural-up-to-date': 'deterministicModuleStructuralUpToDate',
  'structural-tail-rebuilt': 'deterministicModuleStructuralTailRebuilt',
  'structural-rebuilt': 'deterministicModuleStructuralRebuilt',
  'rebuild-scope-empty': 'deterministicModuleRebuildScopeEmpty',
  'rebuild-legacy-fallback': 'deterministicModuleRebuildLegacyFallback',
  'rebuild-checkpoint-aware': 'deterministicModuleRebuildCheckpointAware',
  'rebuild-completed': 'deterministicModuleRebuildCompleted',
  'modules-in-sync': 'deterministicModuleModulesInSync',
  'module-never-built': 'deterministicModuleNeverBuilt',
  'module-no-successful-rebuild': 'deterministicModuleNoSuccessfulRebuild',
  'module-disabled': 'deterministicModuleDisabledNote',
  'module-rebuild-required': 'deterministicModuleRebuildRequired',
  'module-version-mismatch': 'deterministicModuleVersionMismatch',
}

/**
 * Maps stale-reason codes onto shipped Settings copy.
 */
const DERIVED_STALE_KEY_BY_CODE: Record<string, string> = {
  'module-version-changed': 'deterministicModuleStaleVersionChanged',
  'missing-build-timestamp': 'deterministicModuleStaleMissingBuildTimestamp',
  'archive-data-changed': 'deterministicModuleStaleArchiveDataChanged',
  'visibility-or-rollback-changed':
    'deterministicModuleStaleVisibilityOrRollbackChanged',
}

/**
 * Maps a stable `RebuildMode` id onto its localized label.
 *
 * The backend sends the id (never its English label) so the interpolated word
 * inside a translated sentence is translated too.
 */
const REBUILD_MODE_KEY_BY_ID: Record<string, string> = {
  'visit-derive': 'rebuildModeVisitDerive',
  'daily-rollup': 'rebuildModeDailyRollup',
  'structural-rebuild': 'rebuildModeStructuralRebuild',
  'full-rebuild': 'rebuildModeFullRebuild',
}

/**
 * Resolves one coded deterministic-runtime note into user-visible copy.
 *
 * The raw `message` stays the honest fallback for codes this build ships no copy
 * for, so a newer backend never renders a blank line.
 */
function localizeDeterministicRuntimeNote(
  note: DerivedRuntimeNote,
  settingsNs: Translate,
): string {
  const key = DERIVED_NOTE_KEY_BY_CODE[note.code]
  const jobKindKey = REBUILD_MODE_KEY_BY_ID[note.jobKind ?? '']
  // An unresolvable interpolation would leave a hole in the sentence, so fall
  // back to the diagnostic prose rather than ship a half-rendered string.
  if (!key || (note.jobKind && !jobKindKey)) return note.message
  return settingsNs(key, {
    profile: note.profileId ?? '',
    jobKind: jobKindKey ? settingsNs(jobKindKey) : '',
  })
}

/**
 * Resolves one module stale reason into user-visible copy.
 */
function localizeDeterministicStaleReason(
  staleReason: string,
  staleReasonCode: string | null | undefined,
  settingsNs: Translate,
): string {
  const key = DERIVED_STALE_KEY_BY_CODE[staleReasonCode ?? '']
  return key ? settingsNs(key) : staleReason
}

/**
 * Projects a module runtime row onto the note list the card renders.
 *
 * Legacy runtime rows written before code-ification carry only `notes`, so those
 * are surfaced as uncoded pass-throughs instead of being guessed at.
 */
function runtimeNotesFor(
  runtime: { notes: string[]; noteDetails?: DerivedRuntimeNote[] } | undefined,
): DerivedRuntimeNote[] {
  if (!runtime) return []
  if (runtime.noteDetails && runtime.noteDetails.length > 0) {
    return runtime.noteDetails
  }
  return runtime.notes.map((message) => ({ code: '', message }))
}

/**
 * Props for the extracted runtime review surface.
 */
export interface DerivedRuntimeReviewProps {
  action: string | null
  clearReport: ClearDerivedIntelligenceReport | null
  dashboardRecentRun: DashboardSnapshot['recentRuns'][number] | null
  intelligenceRuntime: IntelligenceRuntimeSnapshot | null
  intelligenceRuntimeError: string | null
  rebuildQueueReport: CoreIntelligenceQueueReport | null
  snapshot: AppSnapshot
  onCancelRuntimeJob: (jobId: number) => Promise<void>
  onDeterministicModuleToggle: (moduleId: string) => Promise<void>
  onEnrichmentPluginToggle: (pluginId: string) => Promise<void>
  onRetryRuntimeJob: (jobId: number) => Promise<void>
}

/**
 * Renders runtime/module/plugin review cards from route-owned state.
 */
export function DerivedRuntimeReview({
  action,
  clearReport,
  dashboardRecentRun,
  intelligenceRuntime,
  intelligenceRuntimeError,
  rebuildQueueReport,
  snapshot,
  onDeterministicModuleToggle,
  onEnrichmentPluginToggle,
}: DerivedRuntimeReviewProps) {
  const { language, t, ns } = useI18n()
  const commonNs = ns('common')
  const settingsNs = ns('settings')
  const enrichmentSettings = useMemo(
    () => resolveEnrichmentSettings(snapshot.config.enrichment),
    [snapshot.config.enrichment],
  )
  const runtimePluginsById = useMemo(
    () =>
      new Map(
        (intelligenceRuntime?.plugins ?? []).map((plugin) => [
          plugin.pluginId,
          plugin,
        ]),
      ),
    [intelligenceRuntime?.plugins],
  )
  const reviewableEnrichmentPlugins = useMemo(() => {
    const registryIds = enrichmentPluginRegistry.map((plugin) => plugin.id)
    const extraIds = enrichmentSettings.plugins
      .map((plugin) => plugin.id)
      .filter((pluginId) => !registryIds.includes(pluginId))

    return [...registryIds, ...extraIds].map((pluginId) => ({
      definition: enrichmentPluginRegistry.find(
        (plugin) => plugin.id === pluginId,
      ),
      runtime: runtimePluginsById.get(pluginId),
      state: enrichmentPluginState(enrichmentSettings, pluginId),
    }))
  }, [enrichmentSettings, runtimePluginsById])
  const runtimeModulesById = useMemo(
    () =>
      new Map(
        (intelligenceRuntime?.modules ?? []).map((module) => [
          module.moduleId,
          module,
        ]),
      ),
    [intelligenceRuntime?.modules],
  )
  const reviewableDeterministicModules = useMemo(() => {
    const configuredModules = snapshot.config.deterministic.modules
    const configIds = configuredModules.map((module) => module.id)
    const extraIds = [...runtimeModulesById.keys()].filter(
      (moduleId) => !configIds.includes(moduleId),
    )

    return [...configIds, ...extraIds].map((moduleId) => ({
      notes: runtimeNotesFor(runtimeModulesById.get(moduleId)),
      runtime: runtimeModulesById.get(moduleId),
      state: configuredModules.find((module) => module.id === moduleId) ?? {
        id: moduleId,
        enabled: true,
        version: 'diagnostic',
      },
    }))
  }, [runtimeModulesById, snapshot.config.deterministic.modules])

  return (
    <>
      <StatusCallout
        tone={
          intelligenceRuntimeError || intelligenceRuntime?.queue.failed
            ? 'warning'
            : 'info'
        }
        title={
          intelligenceRuntimeError
            ? settingsNs('runtimeUnavailableTitle')
            : settingsNs('runtimeQueueTitle')
        }
        body={intelligenceRuntimeError ?? settingsNs('runtimeQueueBody')}
        actions={
          intelligenceRuntimeError ? undefined : (
            <div className="settings-action-row">
              <span className="mono">
                {settingsNs('runtimeQueueSummary', {
                  queued: intelligenceRuntime?.queue.queued ?? 0,
                  running: intelligenceRuntime?.queue.running ?? 0,
                  failed: intelligenceRuntime?.queue.failed ?? 0,
                })}
              </span>
            </div>
          )
        }
      />

      {reviewableDeterministicModules.map((module) => (
        <ReviewRuntimeBoundaryCard
          active
          actions={
            <button
              className="btn-secondary"
              type="button"
              disabled={Boolean(action)}
              onClick={() => {
                void onDeterministicModuleToggle(module.state.id)
              }}
            >
              {module.state.enabled
                ? t('settings.disablePlugin')
                : t('settings.enablePlugin')}
            </button>
          }
          description={deterministicModuleDescription(
            module.state.id,
            settingsNs,
          )}
          headerMeta={
            <span className="mono">
              {module.runtime
                ? deterministicModuleStatusLabel(
                    module.runtime.status,
                    settingsNs,
                  )
                : module.state.enabled
                  ? settingsNs('deterministicModuleIdle')
                  : settingsNs('deterministicModuleDisabled')}
            </span>
          }
          key={module.state.id}
          metrics={[
            {
              label: settingsNs('deterministicModuleDependsOn'),
              value: module.runtime?.dependsOn.length
                ? module.runtime.dependsOn
                    .map((moduleId) =>
                      deterministicModuleLabel(moduleId, settingsNs),
                    )
                    .join(', ')
                : commonNs('notAvailable'),
              valueClassName: 'mono',
            },
            {
              label: settingsNs('deterministicModuleTables'),
              value:
                module.runtime?.derivedTables.join(', ') ??
                commonNs('notAvailable'),
              valueClassName: 'mono',
            },
            {
              label: settingsNs('deterministicModuleLastBuilt'),
              value: module.runtime?.lastBuiltAt
                ? (formatDateTime(module.runtime.lastBuiltAt, language) ??
                  module.runtime.lastBuiltAt)
                : commonNs('notAvailable'),
              valueClassName: 'mono',
            },
            ...(module.runtime?.staleReason
              ? [
                  {
                    label: settingsNs('deterministicModuleStaleReason'),
                    value: localizeDeterministicStaleReason(
                      module.runtime.staleReason,
                      module.runtime.staleReasonCode,
                      settingsNs,
                    ),
                  },
                ]
              : []),
          ]}
          notes={
            module.notes.length > 0 ? (
              <div className="intelligence-note-list">
                {module.notes.map((note, index) => (
                  <p
                    className="mono-support"
                    key={`${module.state.id}-${index}-${note.code || note.message}`}
                  >
                    {localizeDeterministicRuntimeNote(note, settingsNs)}
                  </p>
                ))}
              </div>
            ) : undefined
          }
          title={deterministicModuleLabel(module.state.id, settingsNs)}
        />
      ))}

      {reviewableEnrichmentPlugins.map((plugin) => {
        const sourceKind =
          plugin.runtime?.sourceKind ??
          (plugin.state.id === READABLE_CONTENT_REFETCH_PLUGIN_ID
            ? 'network'
            : 'local')

        return (
          <ReviewRuntimeBoundaryCard
            active
            actions={
              <button
                className="btn-secondary"
                type="button"
                disabled={Boolean(action)}
                onClick={() => {
                  void onEnrichmentPluginToggle(plugin.state.id)
                }}
              >
                {plugin.state.enabled
                  ? t('settings.disablePlugin')
                  : t('settings.enablePlugin')}
              </button>
            }
            description={enrichmentPluginDescription(
              plugin.state.id,
              settingsNs,
            )}
            headerMeta={
              <span className="mono">
                {plugin.state.enabled
                  ? t('settings.enabled')
                  : t('settings.disabled')}
              </span>
            }
            key={plugin.state.id}
            metrics={[
              {
                label: settingsNs('pluginBoundary'),
                value: enrichmentPluginBoundaryLabel(sourceKind, settingsNs),
                valueClassName: 'mono',
              },
              {
                label: t('settings.pluginQueue'),
                value: plugin.runtime
                  ? settingsNs('pluginQueueCounts', {
                      queued: plugin.runtime.queuedJobs,
                      running: plugin.runtime.runningJobs,
                      failed: plugin.runtime.failedJobs,
                    })
                  : commonNs('notAvailable'),
                valueClassName: 'mono',
              },
              {
                label: t('settings.pluginFreshness'),
                value: plugin.definition?.freshnessDays
                  ? t('settings.daysFreshness', {
                      days: plugin.definition.freshnessDays,
                    })
                  : commonNs('notAvailable'),
                valueClassName: 'mono',
              },
              {
                label: t('settings.pluginDerivedTables'),
                value:
                  plugin.definition?.derivedTables.join(', ') ??
                  commonNs('notAvailable'),
                valueClassName: 'mono',
              },
              {
                label: settingsNs('pluginStoredRecords'),
                value: plugin.runtime?.storedRecords ?? 0,
                valueClassName: 'mono',
              },
              {
                label: settingsNs('pluginLastCompleted'),
                value: plugin.runtime?.lastCompletedAt
                  ? (formatDateTime(plugin.runtime.lastCompletedAt, language) ??
                    plugin.runtime.lastCompletedAt)
                  : commonNs('notAvailable'),
                valueClassName: 'mono',
              },
              {
                label: settingsNs('pluginLastError'),
                value: plugin.runtime?.lastError ?? commonNs('notAvailable'),
              },
            ]}
            title={enrichmentPluginLabel(plugin.state.id, settingsNs)}
          />
        )
      })}

      <div className="settings-result-list">
        <div className="result-row">
          <div className="result-row__header">
            <strong>{settingsNs('runtimeQueueDetailsTitle')}</strong>
            <Link className="btn-tiny" to="/jobs">
              {settingsNs('runtimeQueueTitle')}
            </Link>
          </div>
          <p>{settingsNs('runtimeQueueDetailsBody')}</p>
        </div>
      </div>

      <div className="settings-result-list">
        {dashboardRecentRun ? (
          <div className="result-row">
            <div className="result-row__header">
              <strong>{t('settings.latestGrowthSignal')}</strong>
              <Link
                className="btn-tiny"
                to={`/audit?run=${dashboardRecentRun.id}`}
              >
                {t('settings.openAuditRun')}
              </Link>
            </div>
            <p>
              {t('settings.latestGrowthSignalBody', {
                runId: dashboardRecentRun.id,
                visits: dashboardRecentRun.newVisits,
                urls: dashboardRecentRun.newUrls,
                downloads: dashboardRecentRun.newDownloads,
              })}
            </p>
          </div>
        ) : null}
        {rebuildQueueReport ? (
          <div className="result-row">
            <div className="result-row__header">
              <strong>{t('settings.rebuildQueuedTitle')}</strong>
              <span className="mono">#{rebuildQueueReport.jobId}</span>
            </div>
            <p>
              {t('settings.rebuildQueuedBody', {
                jobId: rebuildQueueReport.jobId,
              })}
            </p>
            <div className="settings-action-row">
              <Link className="btn-secondary" to="/jobs">
                {t('settings.runtimeQueueTitle')}
              </Link>
            </div>
          </div>
        ) : null}
        {clearReport ? (
          <div className="result-row">
            <div className="result-row__header">
              <strong>{t('settings.clearCompletedTitle')}</strong>
              <span className="mono">
                {clearReport.clearedVisitDerivedFactRows +
                  clearReport.clearedDailyRollupRows +
                  clearReport.clearedStructuralRows +
                  clearReport.clearedRuntimeRows}
              </span>
            </div>
            <p>
              {t('settings.clearCompletedBody', {
                visitDerivedFacts: clearReport.clearedVisitDerivedFactRows,
                dailyRollups: clearReport.clearedDailyRollupRows,
                structural: clearReport.clearedStructuralRows,
                runtime: clearReport.clearedRuntimeRows,
              })}
            </p>
          </div>
        ) : null}
        {action ? <StatusCallout tone="info" title={action} body="" /> : null}
      </div>
    </>
  )
}
