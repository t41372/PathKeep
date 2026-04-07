import { useEffect, useState } from 'react'
import { BusyOverlay } from '../../components/primitives/busy-overlay'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useShellData } from '../../app/shell-data-context'
import { backend } from '../../lib/backend'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  platformLabelKey,
  platformSummaryKey,
} from '../../lib/platform-guidance'
import { scheduleInstallTone } from '../../lib/trust-review'
import type { ApplyResult, SchedulePlan, ScheduleStatus } from '../../lib/types'

interface ScheduleLoadState {
  requestKey: number
  plan: SchedulePlan | null
  status: ScheduleStatus | null
  error: string | null
}

type PmeTab = 'preview' | 'manual' | 'execute'

interface ScheduleExecutionState {
  mode: 'apply' | 'remove'
  result: ApplyResult
}

function waitForNextPaint() {
  return new Promise<void>((resolve) => {
    if (typeof window === 'undefined') {
      resolve()
      return
    }
    window.requestAnimationFrame(() => resolve())
  })
}

function joinCommand(command: string[]) {
  return command
    .map((part) => (part.includes(' ') ? `"${part}"` : part))
    .join(' ')
}

export function SchedulePage() {
  const { refreshAppData, refreshKey, snapshot } = useShellData()
  const { language, t } = useI18n()
  const [loadState, setLoadState] = useState<ScheduleLoadState>({
    requestKey: -1,
    plan: null,
    status: null,
    error: null,
  })
  const [selectedFileIndex, setSelectedFileIndex] = useState(0)
  const [pmeTab, setPmeTab] = useState<PmeTab>('preview')
  const [executionResult, setExecutionResult] =
    useState<ScheduleExecutionState | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
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
          setSelectedFileIndex(0)
        }
      } catch (nextError) {
        if (!cancelled)
          setLoadState({
            requestKey: refreshKey,
            plan: null,
            status: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : t('schedule.unavailableBody'),
          })
      }
    }
    void loadSchedule()
    return () => {
      cancelled = true
    }
  }, [refreshKey, t])

  const plan = loadState.requestKey === refreshKey ? loadState.plan : null
  const status = loadState.requestKey === refreshKey ? loadState.status : null
  const error = loadState.requestKey === refreshKey ? loadState.error : null
  const loading = loadState.requestKey !== refreshKey
  const selectedFile = plan?.generatedFiles[selectedFileIndex] ?? null
  const lastBackup =
    status?.lastSuccessfulBackupAt ??
    snapshot?.archiveStatus.lastSuccessfulBackupAt

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
          <div className="pme-tabs">
            {(['preview', 'manual', 'execute'] as PmeTab[]).map((tab) => (
              <button
                aria-pressed={pmeTab === tab}
                key={tab}
                className={`pme-tab ${pmeTab === tab ? 'active' : ''}`}
                type="button"
                onClick={() => setPmeTab(tab)}
              >
                {tab === 'preview'
                  ? t('common.previewTab')
                  : tab === 'manual'
                    ? t('common.manualTab')
                    : t('common.executeTab')}
              </button>
            ))}
          </div>
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
                <>
                  <div
                    className="generated-file-tabs"
                    style={{ marginBottom: 'var(--space-3)' }}
                  >
                    {plan.generatedFiles.map((file, index) => (
                      <button
                        key={file.relativePath}
                        className={`chip-button ${
                          selectedFileIndex === index
                            ? 'chip-button--active'
                            : ''
                        }`}
                        type="button"
                        onClick={() => setSelectedFileIndex(index)}
                      >
                        {file.relativePath}
                      </button>
                    ))}
                  </div>
                  {selectedFile && (
                    <div className="code-panel">
                      <div className="row-between">
                        <strong>{selectedFile.purpose}</strong>
                        <span className="mono dim">
                          {selectedFile.relativePath}
                        </span>
                      </div>
                      <pre className="code-block">
                        <code>{selectedFile.contents}</code>
                      </pre>
                      {selectedFile.absolutePath && (
                        <div className="code-actions">
                          <button
                            className="btn-tiny"
                            type="button"
                            onClick={() => {
                              void backend.openPathInFileManager(
                                selectedFile.absolutePath ??
                                  selectedFile.relativePath,
                              )
                            }}
                          >
                            {t('common.openPath')}
                          </button>
                        </div>
                      )}
                    </div>
                  )}
                </>
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
                  <span className="step-num-inline mono">
                    {String(index + 1).padStart(2, '0')}
                  </span>
                  <span>{step}</span>
                </div>
              ))}
              {status.detectedFiles.map((path) => (
                <div key={path} className="manual-step">
                  <span className="step-num-inline mono">
                    {t('common.fileStepLabel')}
                  </span>
                  <span className="mono">{path}</span>
                </div>
              ))}
              {status.auditPath && (
                <div className="code-actions">
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => {
                      void backend.openPathInFileManager(status.auditPath ?? '')
                    }}
                  >
                    {t('schedule.openLatestAudit')}
                  </button>
                </div>
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
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => {
                      void backend.openPathInFileManager(
                        executionResult.result.auditPath ?? '',
                      )
                    }}
                  >
                    {t('schedule.openSchedulerAudit')}
                  </button>
                ) : null}
              </div>
              {!snapshot?.config.initialized && (
                <p className="mono-support">
                  {t('schedule.initializeArchiveFirst')}
                </p>
              )}
            </div>
          )}
        </div>
      </div>
      {busy ? <BusyOverlay label={busy} /> : null}
    </section>
  )
}
