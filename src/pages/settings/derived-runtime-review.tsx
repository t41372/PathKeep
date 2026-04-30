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
import { readableContentFetchAvailable } from '../../lib/release-capabilities'
import type {
  AppSnapshot,
  ClearDerivedIntelligenceReport,
  DashboardSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'
import type { CoreIntelligenceQueueReport } from '../../lib/core-intelligence/types'

type Translate = (key: string, vars?: Record<string, string | number>) => string

function localizeDeterministicRuntimeNote(
  note: string,
  settingsNs: Translate,
): string {
  const profilePatterns: Array<[RegExp, string]> = [
    [
      /^No visible visits remained for (.+); cleared visit-derived facts\.$/,
      'deterministicModuleNoVisibleVisitsClearedVisitFacts',
    ],
    [
      /^Visit-derived facts for (.+) were already up to date\.$/,
      'deterministicModuleVisitFactsUpToDate',
    ],
    [
      /^Incrementally refreshed visit-derived facts for (.+)\.$/,
      'deterministicModuleVisitFactsRefreshed',
    ],
    [
      /^Rebuilt visit-derived facts for (.+) with a scoped full refresh\.$/,
      'deterministicModuleVisitFactsRebuilt',
    ],
    [
      /^No visible visits remained for (.+); cleared daily rollups\.$/,
      'deterministicModuleNoVisibleVisitsClearedDailyRollups',
    ],
    [
      /^Daily rollups for (.+) were already up to date\.$/,
      'deterministicModuleDailyRollupsUpToDate',
    ],
    [
      /^Refreshed dirty daily rollups for (.+)\.$/,
      'deterministicModuleDailyRollupsRefreshed',
    ],
    [
      /^Rebuilt all daily rollups for (.+)\.$/,
      'deterministicModuleDailyRollupsRebuilt',
    ],
    [
      /^No visible visits remained for (.+); cleared structural entities\.$/,
      'deterministicModuleNoVisibleVisitsClearedStructural',
    ],
    [
      /^Structural entities for (.+) were already up to date\.$/,
      'deterministicModuleStructuralUpToDate',
    ],
    [
      /^Rebuilt structural tail entities for (.+)\.$/,
      'deterministicModuleStructuralTailRebuilt',
    ],
    [
      /^Rebuilt all structural entities for (.+)\.$/,
      'deterministicModuleStructuralRebuilt',
    ],
  ]

  for (const [pattern, key] of profilePatterns) {
    const match = note.match(pattern)
    if (match) {
      return settingsNs(key, { profile: match[1] })
    }
  }

  if (note === 'Manual full rebuild requested for daily rollups.') {
    return settingsNs('deterministicModuleDailyRollupsManualRebuild')
  }
  if (
    note ===
    'Archive visibility regressed or source counters moved backwards for daily rollups.'
  ) {
    return settingsNs('deterministicModuleDailyRollupsVisibilityRegressed')
  }

  return note
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
  readableContentAvailable?: boolean
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
  readableContentAvailable = readableContentFetchAvailable,
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
                    value: localizeDeterministicRuntimeNote(
                      module.runtime.staleReason,
                      settingsNs,
                    ),
                  },
                ]
              : []),
          ]}
          notes={
            module.runtime?.notes.length ? (
              <div className="intelligence-note-list">
                {module.runtime.notes.map((note) => (
                  <p
                    className="mono-support"
                    key={`${module.state.id}-${note}`}
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
        const readableContentDeferred =
          plugin.state.id === READABLE_CONTENT_REFETCH_PLUGIN_ID &&
          !readableContentAvailable
        const sourceKind =
          plugin.runtime?.sourceKind ??
          (plugin.state.id === READABLE_CONTENT_REFETCH_PLUGIN_ID
            ? 'network'
            : 'local')

        return (
          <ReviewRuntimeBoundaryCard
            active={!readableContentDeferred}
            actions={
              <button
                className="btn-secondary"
                type="button"
                disabled={Boolean(action) || readableContentDeferred}
                title={
                  readableContentDeferred
                    ? settingsNs('readableContentDeferredTooltip')
                    : undefined
                }
                onClick={() => {
                  void onEnrichmentPluginToggle(plugin.state.id)
                }}
              >
                {readableContentDeferred
                  ? t('settings.enablePlugin')
                  : plugin.state.enabled
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
                {readableContentDeferred
                  ? settingsNs('readableContentDeferredBadge')
                  : plugin.state.enabled
                    ? t('settings.enabled')
                    : t('settings.disabled')}
              </span>
            }
            key={plugin.state.id}
            metrics={[
              {
                label: settingsNs('pluginBoundary'),
                value: readableContentDeferred
                  ? settingsNs('readableContentDeferredBadge')
                  : enrichmentPluginBoundaryLabel(sourceKind, settingsNs),
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
