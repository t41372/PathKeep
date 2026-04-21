/**
 * This module renders the Schedule route and keeps the Preview / Manual / Execute / Verify grammar readable across platforms.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `SchedulePage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useEffect, useState } from 'react'
import { BusyOverlay } from '../../components/primitives/busy-overlay'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import {
  copyReviewValue,
  GeneratedArtifactViewer,
  PmeTabBar,
  ReviewPathActionRow,
  type ReviewCopyFeedback,
  VerifyCheckList,
} from '../../components/review'
import { useShellData } from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import { waitForNextPaint } from '../../lib/wait-for-next-paint'
import {
  platformLabelKey,
  platformSummaryKey,
} from '../../lib/platform-guidance'
import { scheduleInstallTone } from '../../lib/trust-review'
import type { ApplyResult, SchedulePlan, ScheduleStatus } from '../../lib/types'

/**
 * Captures the state shape used by `ScheduleLoad`.
 *
 * Keeping this as a named declaration makes the Schedule surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface ScheduleLoadState {
  requestKey: number
  plan: SchedulePlan | null
  status: ScheduleStatus | null
  error: string | null
}

/**
 * Enumerates the tabs available on this front-end surface.
 *
 * Keeping this as a named declaration makes the Schedule surface easier to review and test than burying the behavior inside another anonymous callback.
 */
type PmeTab = 'preview' | 'manual' | 'execute' | 'verify'

/**
 * Captures the state shape used by `ScheduleExecution`.
 *
 * Keeping this as a named declaration makes the Schedule surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface ScheduleExecutionState {
  mode: 'apply' | 'remove'
  result: ApplyResult
}

/**
 * Explains how join command works.
 *
 * Keeping this as a named declaration makes the Schedule surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function joinCommand(command: string[]) {
  return command
    .map((part) => (part.includes(' ') ? `"${part}"` : part))
    .join(' ')
}

async function copyGeneratedArtifact(
  key: string,
  value: string,
  onFeedback: (feedback: ReviewCopyFeedback) => void,
) {
  await copyReviewValue(value, {
    key,
    onFeedback,
  })
}

/**
 * Renders the schedule route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Schedule expectations in the design docs.
 */
export function SchedulePage() {
  const { refreshAppData, refreshKey, snapshot } = useShellData()
  const { language, t } = useI18n()
  const [loadState, setLoadState] = useState<ScheduleLoadState>({
    requestKey: -1,
    plan: null,
    status: null,
    error: null,
  })
  const [copyFeedback, setCopyFeedback] = useState<ReviewCopyFeedback | null>(
    null,
  )
  const [pmeTab, setPmeTab] = useState<PmeTab>('preview')
  const [executionResult, setExecutionResult] =
    useState<ScheduleExecutionState | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const unavailableBody = t('schedule.unavailableBody')

  useEffect(() => {
    let cancelled = false
    /**
     * Loads schedule.
     *
     * Keeping this as a named declaration makes the Schedule surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadSchedule = async () => {
      try {
        const [nextPlan, nextStatus] = await Promise.all([
          backend.previewSchedule(),
          backend.scheduleStatus(),
        ])
        if (!cancelled) {
          setLoadState({
            requestKey: refreshKey,
            plan: nextPlan,
            status: nextStatus,
            error: null,
          })
          setCopyFeedback(null)
        }
      } catch (nextError) {
        if (!cancelled)
          setLoadState({
            requestKey: refreshKey,
            plan: null,
            status: null,
            error:
              nextError instanceof Error ? nextError.message : unavailableBody,
          })
      }
    }
    void loadSchedule()
    return () => {
      cancelled = true
    }
  }, [refreshKey, unavailableBody])

  const plan = loadState.requestKey === refreshKey ? loadState.plan : null
  const status = loadState.requestKey === refreshKey ? loadState.status : null
  const error = loadState.requestKey === refreshKey ? loadState.error : null
  const loading = loadState.requestKey !== refreshKey
  const lastBackup =
    status?.lastSuccessfulBackupAt ??
    snapshot?.archiveStatus.lastSuccessfulBackupAt
  const latestAuditPath = executionResult?.result.auditPath ?? status?.auditPath

  const badge =
    status?.installState === 'installed'
      ? t('schedule.installedBadge')
      : status?.installState === 'manual-review'
        ? t('schedule.manualReviewBadge')
        : status?.installState === 'not-installed'
          ? t('schedule.notInstalledBadge')
          : t('schedule.attentionBadge')

  const installDescription =
    status?.installState === 'installed'
      ? t('schedule.installedDescription')
      : status?.installState === 'mismatch'
        ? t('schedule.mismatchDescription')
        : status?.installState === 'permission-warning'
          ? t('schedule.permissionWarningDescription')
          : status?.installState === 'legacy-install-detected'
            ? t('schedule.legacyInstallDescription')
            : status?.installState === 'manual-review'
              ? t('schedule.manualReviewDescription')
              : t('schedule.notInstalledDescription')

  /**
   * Handles apply.
   *
   * Keeping this as a named declaration makes the Schedule surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleApply() {
    if (!plan) return

    setBusy(t('schedule.applySchedule'))
    setActionError(null)
    setExecutionResult(null)

    try {
      await waitForNextPaint()
      const result = await backend.applySchedule(plan)
      setExecutionResult({ mode: 'apply', result })
      await refreshAppData()
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    } finally {
      setBusy(null)
    }
  }

  /**
   * Handles remove.
   *
   * Keeping this as a named declaration makes the Schedule surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleRemove() {
    if (!plan) return

    setBusy(t('schedule.removeSchedule'))
    setActionError(null)
    setExecutionResult(null)

    try {
      await waitForNextPaint()
      const result = await backend.removeSchedule(plan)
      setExecutionResult({ mode: 'remove', result })
      await refreshAppData()
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    } finally {
      setBusy(null)
    }
  }

  if (loading && !plan)
    return (
      <section className="page-shell">
        <LoadingState label={t('schedule.loadingPreview')} />
      </section>
    )
  if (error || !plan || !status)
    return (
      <section className="page-shell">
        <ErrorState
          title={t('schedule.unavailableTitle')}
          description={error ?? t('schedule.unavailableBody')}
        />
      </section>
    )

  return (
    <section className="page-shell schedule-page" data-testid="schedule-page">
      <StatusCallout
        tone={scheduleInstallTone(status.installState)}
        title={t(platformLabelKey(status.platform))}
        body={t(platformSummaryKey(status.platform))}
      />
      <StatusCallout
        tone={scheduleInstallTone(status.installState)}
        title={badge}
        body={installDescription}
        actions={
          <div className="intelligence-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setPmeTab('preview')}
            >
              {t('common.previewTab')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setPmeTab('manual')}
            >
              {t('common.manualTab')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setPmeTab('execute')}
            >
              {t('common.executeTab')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setPmeTab('verify')}
            >
              {t('common.verifyTab')}
            </button>
          </div>
        }
      />

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.backupSchedule')}</span>
          <span className="status-badge">{badge}</span>
        </div>
        <div className="panel-body">
          <div className="schedule-config">
            <div className="config-row">
              <span className="config-label">{t('schedule.installState')}</span>
              <span className="config-value">{installDescription}</span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('schedule.interval')}</span>
              <span className="config-value mono">
                {t('schedule.intervalValue', {
                  hours: status.dueAfterHours,
                })}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('schedule.verification')}</span>
              <span className="config-value mono">
                {t('schedule.verificationValue', {
                  hours: status.checkIntervalHours,
                })}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('schedule.mechanism')}</span>
              <span className="config-value mono">
                {t(platformLabelKey(status.platform))}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">
                {t('schedule.lastTriggered')}
              </span>
              <span className="config-value mono">
                {lastBackup
                  ? formatRelativeTime(lastBackup, language)
                  : t('common.notAvailable')}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('schedule.label')}</span>
              <span className="config-value mono">{status.label}</span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('schedule.profiles')}</span>
              <span className="config-value">
                {snapshot?.config.selectedProfileIds.join(', ') ??
                  t('common.notAvailable')}
              </span>
            </div>
          </div>

          {status.warnings.map((warning) => (
            <div key={warning} className="warning-box">
              <div className="warning-icon">⚠</div>
              <div className="warning-text">{warning}</div>
            </div>
          ))}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('schedule.pmeTitle')}</span>
          <PmeTabBar
            activeTab={pmeTab}
            onChange={setPmeTab}
            tabs={[
              { key: 'preview', label: t('common.previewTab') },
              { key: 'manual', label: t('common.manualTab') },
              { key: 'execute', label: t('common.executeTab') },
              { key: 'verify', label: t('common.verifyTab') },
            ]}
          />
        </div>
        <div className="panel-body">
          {pmeTab === 'preview' && (
            <>
              <div className="summary-label">
                {t('schedule.previewBoundary')}
              </div>
              <p className="dashboard-next-action">
                {t('schedule.previewBody')}
              </p>
              {plan.generatedFiles.length > 0 ? (
                <GeneratedArtifactViewer
                  copyFeedback={copyFeedback}
                  copyLabel={t('common.copyAction')}
                  copyPathLabel={t('common.copyAction')}
                  errorMessage={t('audit.copyFailed')}
                  files={plan.generatedFiles}
                  onCopy={(key, value) => {
                    void copyGeneratedArtifact(key, value, setCopyFeedback)
                  }}
                  onOpenPath={(path) => {
                    void backend.openPathInFileManager(path)
                  }}
                  openPathLabel={t('common.openPath')}
                  successMessage={t('common.copiedNotice')}
                />
              ) : (
                <div className="dim" style={{ fontSize: '12px' }}>
                  {t('schedule.noGeneratedFiles')}
                </div>
              )}
            </>
          )}

          {pmeTab === 'manual' && (
            <div className="manual-steps">
              {status.manualSteps.map((step, index) => (
                <div key={step} className="manual-step">
                  <span className="step-num-inline mono">{index + 1}</span>
                  <span>{step}</span>
                </div>
              ))}
              {status.detectedFiles.map((path) => (
                <ReviewPathActionRow
                  key={path}
                  copyFeedback={copyFeedback}
                  copyKey={`schedule:detected:${path}`}
                  copyLabel={t('common.copyAction')}
                  errorMessage={t('audit.copyFailed')}
                  label={t('common.fileStepLabel')}
                  onCopy={(key, value) => {
                    void copyReviewValue(value, {
                      key,
                      onFeedback: setCopyFeedback,
                    })
                  }}
                  onOpenPath={(nextPath) => {
                    void backend.openPathInFileManager(nextPath)
                  }}
                  openPathLabel={t('common.openPath')}
                  successMessage={t('common.copiedNotice')}
                  value={path}
                />
              ))}
              {status.auditPath && (
                <ReviewPathActionRow
                  copyFeedback={copyFeedback}
                  copyKey="schedule:audit-path"
                  copyLabel={t('common.copyAction')}
                  errorMessage={t('audit.copyFailed')}
                  label={t('schedule.openLatestAudit')}
                  onCopy={(key, value) => {
                    void copyReviewValue(value, {
                      key,
                      onFeedback: setCopyFeedback,
                    })
                  }}
                  onOpenPath={(path) => {
                    void backend.openPathInFileManager(path)
                  }}
                  openPathLabel={t('common.openPath')}
                  successMessage={t('common.copiedNotice')}
                  value={status.auditPath}
                />
              )}
            </div>
          )}

          {pmeTab === 'execute' && (
            <div className="manual-steps">
              <div className="manual-step">
                <span className="step-num-inline mono">
                  {t('schedule.executeRun')}
                </span>
                <span>{t('schedule.executeBody')}</span>
              </div>
              {plan.applyCommands.map((command, index) => (
                <div
                  key={`${command.join(' ')}-${index}`}
                  className="code-panel"
                >
                  <div className="summary-label">
                    {t('schedule.applyCommand', { index: index + 1 })}
                  </div>
                  <pre className="code-block">
                    <code>{joinCommand(command)}</code>
                  </pre>
                </div>
              ))}
              {plan.rollbackCommands.map((command, index) => (
                <div
                  key={`${command.join(' ')}-rollback-${index}`}
                  className="code-panel"
                >
                  <div className="summary-label">
                    {t('schedule.rollbackCommand', { index: index + 1 })}
                  </div>
                  <pre className="code-block">
                    <code>{joinCommand(command)}</code>
                  </pre>
                </div>
              ))}
              {actionError ? (
                <p className="inline-error" role="alert">
                  {actionError}
                </p>
              ) : null}
              {executionResult ? (
                <div className="warning-box">
                  <div className="warning-icon">ℹ</div>
                  <div className="warning-text">
                    <strong>
                      {executionResult.mode === 'apply'
                        ? t('schedule.applySchedule')
                        : t('schedule.removeSchedule')}
                    </strong>{' '}
                    {executionResult.result.message}
                  </div>
                </div>
              ) : null}
              <div className="wizard-actions">
                <button
                  className="btn-primary"
                  type="button"
                  disabled={
                    busy !== null ||
                    !snapshot?.config.initialized ||
                    !plan.applySupported
                  }
                  onClick={() => {
                    void handleApply()
                  }}
                >
                  {busy === t('schedule.applySchedule')
                    ? busy
                    : t('schedule.applySchedule')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  disabled={
                    busy !== null ||
                    !plan.applySupported ||
                    (status.installState === 'not-installed' &&
                      status.detectedFiles.length === 0)
                  }
                  onClick={() => {
                    void handleRemove()
                  }}
                >
                  {busy === t('schedule.removeSchedule')
                    ? busy
                    : t('schedule.removeSchedule')}
                </button>
                {executionResult?.result.auditPath ? (
                  <ReviewPathActionRow
                    copyFeedback={copyFeedback}
                    copyKey="schedule:execution-audit"
                    copyLabel={t('common.copyAction')}
                    errorMessage={t('audit.copyFailed')}
                    label={t('schedule.openSchedulerAudit')}
                    onCopy={(key, value) => {
                      void copyReviewValue(value, {
                        key,
                        onFeedback: setCopyFeedback,
                      })
                    }}
                    onOpenPath={(path) => {
                      void backend.openPathInFileManager(path)
                    }}
                    openPathLabel={t('common.openPath')}
                    successMessage={t('common.copiedNotice')}
                    value={executionResult.result.auditPath}
                  />
                ) : null}
              </div>
              {!snapshot?.config.initialized && (
                <p className="mono-support">
                  {t('schedule.initializeArchiveFirst')}
                </p>
              )}
            </div>
          )}

          {pmeTab === 'verify' && (
            <div className="settings-result-list">
              <div className="summary-label">{t('common.verifyTab')}</div>
              <p className="dashboard-next-action">{installDescription}</p>
              <VerifyCheckList
                items={[
                  {
                    key: 'install-state',
                    label: t('schedule.installState'),
                    status: badge,
                    body: installDescription,
                  },
                  {
                    key: 'last-triggered',
                    label: t('schedule.lastTriggered'),
                    status: lastBackup
                      ? formatRelativeTime(lastBackup, language)
                      : t('common.notAvailable'),
                  },
                  {
                    key: 'detected-files',
                    label: t('common.filesLabel'),
                    status:
                      status.detectedFiles.length > 0
                        ? String(status.detectedFiles.length)
                        : t('common.notAvailable'),
                    body:
                      status.detectedFiles.length > 0
                        ? status.detectedFiles.join(' · ')
                        : undefined,
                  },
                ]}
              />
              {status.detectedFiles.length > 0 ? (
                <div className="manual-steps">
                  {status.detectedFiles.map((path) => (
                    <ReviewPathActionRow
                      key={path}
                      copyFeedback={copyFeedback}
                      copyKey={`schedule:verify-detected:${path}`}
                      copyLabel={t('common.copyAction')}
                      errorMessage={t('audit.copyFailed')}
                      label={t('common.fileStepLabel')}
                      onCopy={(key, value) => {
                        void copyReviewValue(value, {
                          key,
                          onFeedback: setCopyFeedback,
                        })
                      }}
                      onOpenPath={(nextPath) => {
                        void backend.openPathInFileManager(nextPath)
                      }}
                      openPathLabel={t('common.openPath')}
                      successMessage={t('common.copiedNotice')}
                      value={path}
                    />
                  ))}
                </div>
              ) : null}
              {status.warnings.length > 0 ? (
                <div className="warning-box">
                  <div className="warning-icon">⚠</div>
                  <div className="warning-text">
                    {status.warnings.map((warning) => (
                      <div key={warning}>{warning}</div>
                    ))}
                  </div>
                </div>
              ) : (
                <StatusCallout
                  tone="success"
                  title={t('common.statusClear')}
                  body={installDescription}
                />
              )}
              {executionResult ? (
                <div className="warning-box">
                  <div className="warning-icon">ℹ</div>
                  <div className="warning-text">
                    <strong>
                      {executionResult.mode === 'apply'
                        ? t('schedule.applySchedule')
                        : t('schedule.removeSchedule')}
                    </strong>{' '}
                    {executionResult.result.message}
                  </div>
                </div>
              ) : null}
              {latestAuditPath ? (
                <ReviewPathActionRow
                  copyFeedback={copyFeedback}
                  copyKey="schedule:latest-audit"
                  copyLabel={t('common.copyAction')}
                  errorMessage={t('audit.copyFailed')}
                  label={t('schedule.openLatestAudit')}
                  onCopy={(key, value) => {
                    void copyReviewValue(value, {
                      key,
                      onFeedback: setCopyFeedback,
                    })
                  }}
                  onOpenPath={(path) => {
                    void backend.openPathInFileManager(path)
                  }}
                  openPathLabel={t('common.openPath')}
                  successMessage={t('common.copiedNotice')}
                  value={latestAuditPath}
                />
              ) : null}
            </div>
          )}
        </div>
      </div>
      {busy ? <BusyOverlay label={busy} /> : null}
    </section>
  )
}
