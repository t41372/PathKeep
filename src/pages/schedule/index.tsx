import { useEffect, useState } from 'react'
import { useShellData } from '../../app/shell-data-context'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import { formatRelativeTime } from '../../lib/format'
import type { ApplyResult, SchedulePlan, ScheduleStatus } from '../../lib/types'

interface ScheduleLoadState {
  requestKey: number
  plan: SchedulePlan | null
  status: ScheduleStatus | null
  error: string | null
}

type PmeTab = 'preview' | 'manual' | 'execute'

function scheduleBadge(status: ScheduleStatus | null) {
  if (!status) return { className: 'status-pending', label: 'Loading' }

  if (status.installState === 'installed') {
    return { className: 'status-completed', label: 'Installed' }
  }

  if (
    status.installState === 'mismatch' ||
    status.installState === 'permission-warning' ||
    status.installState === 'legacy-install-detected'
  ) {
    return { className: 'status-pending', label: 'Attention' }
  }

  if (status.installState === 'manual-review') {
    return { className: 'status-pending', label: 'Manual review' }
  }

  return { className: 'status-pending', label: 'Not installed' }
}

function describeInstallState(status: ScheduleStatus) {
  if (status.installState === 'installed') {
    return 'Native schedule files match the current PathKeep plan.'
  }

  if (status.installState === 'mismatch') {
    return 'Installed files exist, but they no longer match the current preview.'
  }

  if (status.installState === 'permission-warning') {
    return 'PathKeep could not inspect the installed files cleanly on this machine.'
  }

  if (status.installState === 'legacy-install-detected') {
    return 'A legacy install is still present. Review it before trusting this schedule.'
  }

  if (status.installState === 'manual-review') {
    return 'This platform stays manual-first in v1. Verify it using the documented steps.'
  }

  return 'No installed native schedule was detected yet.'
}

function joinCommand(command: string[]) {
  return command
    .map((part) => (part.includes(' ') ? `"${part}"` : part))
    .join(' ')
}

export function SchedulePage() {
  const { refreshAppData, refreshKey, snapshot } = useShellData()
  const [loadState, setLoadState] = useState<ScheduleLoadState>({
    requestKey: -1,
    plan: null,
    status: null,
    error: null,
  })
  const [selectedFileIndex, setSelectedFileIndex] = useState(0)
  const [pmeTab, setPmeTab] = useState<PmeTab>('preview')
  const [applyResult, setApplyResult] = useState<ApplyResult | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

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
                : 'PathKeep could not preview the native schedule.',
          })
      }
    }
    void loadSchedule()
    return () => {
      cancelled = true
    }
  }, [refreshKey])

  const plan = loadState.requestKey === refreshKey ? loadState.plan : null
  const status = loadState.requestKey === refreshKey ? loadState.status : null
  const error = loadState.requestKey === refreshKey ? loadState.error : null
  const loading = loadState.requestKey !== refreshKey
  const selectedFile = plan?.generatedFiles[selectedFileIndex] ?? null
  const badge = scheduleBadge(status)
  const lastBackup =
    status?.lastSuccessfulBackupAt ??
    snapshot?.archiveStatus.lastSuccessfulBackupAt

  async function handleApply() {
    if (!plan) return

    setBusy(true)
    setActionError(null)
    setApplyResult(null)

    try {
      const result = await backend.applySchedule(plan)
      setApplyResult(result)
      await refreshAppData()
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : 'PathKeep could not apply the native schedule.',
      )
    } finally {
      setBusy(false)
    }
  }

  if (loading && !plan)
    return (
      <section className="page-shell">
        <LoadingState label="Rendering native schedule preview" />
      </section>
    )
  if (error || !plan || !status)
    return (
      <section className="page-shell">
        <ErrorState
          title="Schedule preview unavailable"
          description={
            error ?? 'PathKeep could not render the native schedule artifacts.'
          }
        />
      </section>
    )

  return (
    <section className="page-shell schedule-page" data-testid="schedule-page">
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">BACKUP SCHEDULE</span>
          <span className={`status-badge ${badge.className}`}>
            {badge.label}
          </span>
        </div>
        <div className="panel-body">
          <div className="schedule-config">
            <div className="config-row">
              <span className="config-label">Install State</span>
              <span className="config-value">
                {describeInstallState(status)}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Interval</span>
              <span className="config-value mono">
                Every {status.dueAfterHours} hours
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Verification</span>
              <span className="config-value mono">
                Check every {status.checkIntervalHours} hours
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Mechanism</span>
              <span className="config-value mono">
                {status.platform === 'macos'
                  ? 'macOS LaunchAgent'
                  : status.platform}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Last Triggered</span>
              <span className="config-value mono">
                {lastBackup ? formatRelativeTime(lastBackup) : 'N/A'}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Label</span>
              <span className="config-value mono">{status.label}</span>
            </div>
            <div className="config-row">
              <span className="config-label">Profiles</span>
              <span className="config-value">
                {snapshot?.config.selectedProfileIds.join(', ') ?? 'None'}
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
          <span className="panel-title">SCHEDULE PME</span>
          <div className="pme-tabs">
            <button
              className={`pme-tab ${pmeTab === 'preview' ? 'active' : ''}`}
              type="button"
              onClick={() => setPmeTab('preview')}
            >
              PREVIEW
            </button>
            <button
              className={`pme-tab ${pmeTab === 'manual' ? 'active' : ''}`}
              type="button"
              onClick={() => setPmeTab('manual')}
            >
              MANUAL
            </button>
            <button
              className={`pme-tab ${pmeTab === 'execute' ? 'active' : ''}`}
              type="button"
              onClick={() => setPmeTab('execute')}
            >
              EXECUTE
            </button>
          </div>
        </div>
        <div className="panel-body">
          {pmeTab === 'preview' && (
            <>
              <div className="summary-label">PREVIEW BOUNDARY</div>
              <p className="dashboard-next-action">
                Review the exact artifact PathKeep would install before trusting
                any native automation. This page never assumes the install state
                without reading the platform status surface.
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
                            Open path
                          </button>
                        </div>
                      )}
                    </div>
                  )}
                </>
              ) : (
                <div className="dim" style={{ fontSize: '12px' }}>
                  No generated files are available in browser preview mode. Open
                  the desktop build to inspect the full native artifact.
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
                  <span className="step-num-inline mono">FILE</span>
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
                    Open latest scheduler audit
                  </button>
                </div>
              )}
            </div>
          )}

          {pmeTab === 'execute' && (
            <div className="manual-steps">
              <div className="manual-step">
                <span className="step-num-inline mono">RUN</span>
                <span>
                  Execute installs or updates the current native schedule plan.
                  Review the preview artifact and warnings first.
                </span>
              </div>
              {plan.applyCommands.map((command, index) => (
                <div
                  key={`${command.join(' ')}-${index}`}
                  className="code-panel"
                >
                  <div className="summary-label">APPLY COMMAND {index + 1}</div>
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
                    ROLLBACK COMMAND {index + 1}
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
              {applyResult ? (
                <div className="warning-box">
                  <div className="warning-icon">ℹ</div>
                  <div className="warning-text">
                    <strong>
                      {applyResult.applied
                        ? 'Schedule updated.'
                        : 'Apply stayed read-only.'}
                    </strong>{' '}
                    {applyResult.message}
                  </div>
                </div>
              ) : null}
              <div className="wizard-actions">
                <button
                  className="btn-primary"
                  type="button"
                  disabled={
                    busy ||
                    !snapshot?.config.initialized ||
                    !plan.applySupported
                  }
                  onClick={() => {
                    void handleApply()
                  }}
                >
                  {busy ? 'Applying schedule' : 'Apply schedule'}
                </button>
                {applyResult?.auditPath ? (
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => {
                      void backend.openPathInFileManager(
                        applyResult.auditPath ?? '',
                      )
                    }}
                  >
                    Open apply audit
                  </button>
                ) : null}
              </div>
              {!snapshot?.config.initialized && (
                <p className="mono-support">
                  Initialize the archive first, then return here to apply the
                  reviewed native schedule.
                </p>
              )}
            </div>
          )}
        </div>
      </div>
    </section>
  )
}
