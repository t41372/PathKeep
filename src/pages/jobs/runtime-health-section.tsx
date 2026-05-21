/**
 * @file runtime-health-section.tsx
 * @description Renders the Jobs route runtime-health and runtime-boundary sections around the shared review card grammar.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Keep the Jobs runtime summary, plugin/module focus cards, and runtime-boundary summary rows together.
 * - Adapt runtime snapshot facts into the shared `ReviewRuntimeBoundaryCard` shell.
 * - Leave retry/cancel mutations and route-level gating to `jobs/index.tsx`.
 *
 * ## Non-Responsibilities
 * - Does not fetch runtime state or pause/resume the queue.
 * - Does not render recent AI/runtime job panels.
 * - Does not introduce a second runtime-boundary owner outside `components/review/`.
 *
 * ## Dependencies
 * - Depends on `ReviewRuntimeBoundaryCard` as the shared runtime review shell.
 * - Reuses runtime label helpers from `src/lib/intelligence-runtime.ts`.
 * - Uses Jobs copy helpers from `src/lib/intelligence-presentation.ts` for honest plugin error text.
 *
 * ## Performance Notes
 * - Works entirely from the already-loaded runtime snapshot; it does not trigger additional reads.
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { ReviewRuntimeBoundaryCard } from '../../components/review'
import { formatDateTime } from '../../lib/format'
import {
  summarizePluginError,
  type JobsTranslator,
} from '../../lib/intelligence-presentation'
import {
  deterministicModuleDescription,
  deterministicModuleLabel,
  deterministicModuleStatusLabel,
  enrichmentPluginBoundaryLabel,
  enrichmentPluginDescription,
  enrichmentPluginLabel,
} from '../../lib/intelligence-runtime'
import { readableContentFetchAvailable } from '../../lib/release-capabilities'
import type { ResolvedLanguage } from '../../lib/i18n'
import type { IntelligenceRuntimeSnapshot } from '../../lib/types'

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Describes the props accepted by `JobsRuntimeHealthSection`.
 *
 * The Jobs route owns loading/error state and queue mutations, while this
 * extracted section only needs the already-loaded runtime snapshot and the
 * translators required to render honest summary text.
 */
export interface JobsRuntimeHealthSectionProps {
  commonT: Translator
  jobsT: JobsTranslator
  language: ResolvedLanguage
  runtime: IntelligenceRuntimeSnapshot | null
  settingsT: Translator
}

/**
 * Renders the Jobs runtime-health section around shared runtime-boundary cards.
 *
 * This keeps the route file focused on workflow orchestration while the actual
 * runtime summary shell stays reusable and easier to evolve alongside Settings.
 */
export function JobsRuntimeHealthSection({
  commonT,
  jobsT,
  language,
  runtime,
  settingsT,
}: JobsRuntimeHealthSectionProps) {
  const contentPlugin =
    runtime?.plugins.find(
      (plugin) => plugin.pluginId === 'readable-content-refetch',
    ) ?? null
  const titlePlugin =
    runtime?.plugins.find(
      (plugin) => plugin.pluginId === 'title-normalization',
    ) ?? null
  const readyModuleCount =
    runtime?.modules.filter((module) => module.status === 'ready').length ?? 0
  const attentionModuleCount =
    runtime?.modules.filter((module) => module.status !== 'ready').length ?? 0
  const latestModuleBuildAt = runtime?.modules.reduce<string | null>(
    (latest, module) => {
      if (!module.lastBuiltAt) {
        return latest
      }
      if (!latest || module.lastBuiltAt > latest) {
        return module.lastBuiltAt
      }
      return latest
    },
    null,
  )
  const contentQueueMessage = !readableContentFetchAvailable
    ? jobsT('contentFetchDeferredBody')
    : contentPlugin?.lastError
      ? summarizePluginError(contentPlugin, jobsT)
      : contentPlugin
        ? contentPlugin.queuedJobs > 0
          ? jobsT('contentFetchBacklogBody', {
              queued: contentPlugin.queuedJobs,
              stored: contentPlugin.storedRecords,
            })
          : contentPlugin.runningJobs > 0
            ? jobsT('contentFetchRunningBody', {
                stored: contentPlugin.storedRecords,
              })
            : jobsT('contentFetchReadyBody', {
                stored: contentPlugin.storedRecords,
              })
        : jobsT('contentFetchFallbackBody')
  const visibleContentStats = readableContentFetchAvailable
    ? {
        queued: contentPlugin?.queuedJobs ?? 0,
        running: contentPlugin?.runningJobs ?? 0,
        failed: contentPlugin?.failedJobs ?? 0,
        stored: contentPlugin?.storedRecords ?? 0,
      }
    : {
        queued: 0,
        running: 0,
        failed: 0,
        stored: 0,
      }

  return (
    <>
      <div className="jobs-focus-grid">
        <PaperCard className="jobs-focus-card">
          <PaperCardHeader
            title={jobsT('contentFetchTitle')}
            right={
              <PaperCardBadge>
                {readableContentFetchAvailable
                  ? enrichmentPluginBoundaryLabel('network', settingsT)
                  : jobsT('contentFetchDeferredBadge')}
              </PaperCardBadge>
            }
          />
          <PaperCardBody className="jobs-panel-stack">
            <p>{contentQueueMessage}</p>
            <div className="jobs-meta-grid mono-support">
              <span>
                {jobsT('queuedCount')}:{' '}
                {visibleContentStats.queued.toLocaleString(language)}
              </span>
              <span>
                {jobsT('runningCount')}:{' '}
                {visibleContentStats.running.toLocaleString(language)}
              </span>
              <span>
                {jobsT('failedCount')}:{' '}
                {visibleContentStats.failed.toLocaleString(language)}
              </span>
              <span>
                {jobsT('savedReadableContent')}:{' '}
                {visibleContentStats.stored.toLocaleString(language)}
              </span>
            </div>
            {readableContentFetchAvailable && contentPlugin?.lastError ? (
              <p className="mono-support">
                {summarizePluginError(contentPlugin, jobsT)}
              </p>
            ) : null}
          </PaperCardBody>
        </PaperCard>

        <PaperCard className="jobs-focus-card">
          <PaperCardHeader
            title={enrichmentPluginLabel('title-normalization', settingsT)}
            right={
              <PaperCardBadge>
                {enrichmentPluginBoundaryLabel('local', settingsT)}
              </PaperCardBadge>
            }
          />
          <PaperCardBody className="jobs-panel-stack">
            <p>{jobsT('titleNormalizationBody')}</p>
            <div className="jobs-meta-grid mono-support">
              <span>
                {jobsT('queuedCount')}:{' '}
                {(titlePlugin?.queuedJobs ?? 0).toLocaleString(language)}
              </span>
              <span>
                {jobsT('runningCount')}:{' '}
                {(titlePlugin?.runningJobs ?? 0).toLocaleString(language)}
              </span>
              <span>
                {jobsT('failedCount')}:{' '}
                {(titlePlugin?.failedJobs ?? 0).toLocaleString(language)}
              </span>
              <span>
                {jobsT('storedRecordsLabel')}:{' '}
                {(titlePlugin?.storedRecords ?? 0).toLocaleString(language)}
              </span>
            </div>
          </PaperCardBody>
        </PaperCard>

        <PaperCard className="jobs-focus-card">
          <PaperCardHeader
            title={jobsT('modulesTitle')}
            right={
              <PaperCardBadge>
                {readyModuleCount.toLocaleString(language)} /{' '}
                {(runtime?.modules.length ?? 0).toLocaleString(language)}
              </PaperCardBadge>
            }
          />
          <PaperCardBody className="jobs-panel-stack">
            <p>
              {attentionModuleCount > 0
                ? jobsT('moduleAttentionBody', {
                    count: attentionModuleCount,
                  })
                : jobsT('moduleHealthyBody')}
            </p>
            <div className="jobs-meta-grid mono-support">
              <span>
                {jobsT('moduleReadyCount')}:{' '}
                {readyModuleCount.toLocaleString(language)}
              </span>
              <span>
                {jobsT('moduleAttentionCount')}:{' '}
                {attentionModuleCount.toLocaleString(language)}
              </span>
              <span>
                {jobsT('lastCompletedAt')}:{' '}
                {latestModuleBuildAt
                  ? formatDateTime(latestModuleBuildAt, language)
                  : commonT('notAvailable')}
              </span>
            </div>
          </PaperCardBody>
        </PaperCard>

        <PaperCard className="jobs-focus-card">
          <PaperCardHeader title={jobsT('recoveryTitle')} />
          <PaperCardBody className="jobs-panel-stack">
            <p>{jobsT('recoveryBody')}</p>
            {runtime?.notes?.length ? (
              <div className="jobs-notes">
                {runtime.notes.map((note) => (
                  <p key={note} className="mono-support">
                    {note}
                  </p>
                ))}
              </div>
            ) : (
              <p className="mono-support">{jobsT('noRecoveryNotes')}</p>
            )}
          </PaperCardBody>
        </PaperCard>
      </div>

      <div className="jobs-section-heading">
        <span className="panel-title">{jobsT('runtimeHealthTitle')}</span>
        <p>{jobsT('runtimeHealthBody')}</p>
      </div>

      <div className="jobs-summary-grid">
        <PaperCard>
          <PaperCardHeader title={jobsT('pluginsTitle')} />
          <PaperCardBody className="jobs-status-grid">
            {(runtime?.plugins ?? []).map((plugin) => (
              <ReviewRuntimeBoundaryCard
                key={plugin.pluginId}
                description={enrichmentPluginDescription(
                  plugin.pluginId,
                  settingsT,
                )}
                headerMeta={
                  <span className="mono-support">
                    {enrichmentPluginBoundaryLabel(
                      plugin.sourceKind,
                      settingsT,
                    )}
                  </span>
                }
                metrics={[
                  {
                    label: jobsT('queuedCount'),
                    value: plugin.queuedJobs.toLocaleString(language),
                    valueClassName: 'mono-support',
                  },
                  {
                    label: jobsT('runningCount'),
                    value: plugin.runningJobs.toLocaleString(language),
                    valueClassName: 'mono-support',
                  },
                  {
                    label: jobsT('failedCount'),
                    value: plugin.failedJobs.toLocaleString(language),
                    valueClassName: 'mono-support',
                  },
                  {
                    label: jobsT('lastCompletedAt'),
                    value: plugin.lastCompletedAt
                      ? (formatDateTime(plugin.lastCompletedAt, language) ??
                        plugin.lastCompletedAt)
                      : commonT('notAvailable'),
                    valueClassName: 'mono-support',
                  },
                ]}
                notes={
                  plugin.lastError ? (
                    <p className="mono-support">
                      {summarizePluginError(plugin, jobsT)}
                    </p>
                  ) : undefined
                }
                title={enrichmentPluginLabel(plugin.pluginId, settingsT)}
              />
            ))}
          </PaperCardBody>
        </PaperCard>

        <PaperCard>
          <PaperCardHeader title={jobsT('modulesTitle')} />
          <PaperCardBody className="jobs-status-grid">
            {(runtime?.modules ?? []).map((module) => (
              <ReviewRuntimeBoundaryCard
                key={module.moduleId}
                description={deterministicModuleDescription(
                  module.moduleId,
                  settingsT,
                )}
                headerMeta={
                  <span className="mono-support">
                    {deterministicModuleStatusLabel(module.status, settingsT)}
                  </span>
                }
                metrics={[
                  {
                    label: jobsT('lastCompletedAt'),
                    value: module.lastBuiltAt
                      ? (formatDateTime(module.lastBuiltAt, language) ??
                        module.lastBuiltAt)
                      : commonT('notAvailable'),
                    valueClassName: 'mono-support',
                  },
                  {
                    label: jobsT('derivedTables'),
                    value: module.derivedTables.join(', '),
                    valueClassName: 'mono-support',
                  },
                ]}
                notes={
                  module.staleReason ? (
                    <p className="mono-support">{module.staleReason}</p>
                  ) : undefined
                }
                title={deterministicModuleLabel(module.moduleId, settingsT)}
              />
            ))}
          </PaperCardBody>
        </PaperCard>
      </div>
    </>
  )
}
