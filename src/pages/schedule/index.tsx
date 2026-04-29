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
import { BackupIntervalSelector } from '../../components/schedule/backup-interval-selector'
import {
  copyReviewValue,
  type ReviewCopyFeedback,
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
import type { SchedulePlan, ScheduleStatus } from '../../lib/types'
import {
  SchedulePmePanel,
  type ScheduleExecutionState,
  type SchedulePmeTab,
} from './pme-panel'

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
 * Renders the schedule route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Schedule expectations in the design docs.
 */
export function SchedulePage() {
  const { refreshAppData, refreshKey, saveConfig, snapshot } = useShellData()
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
  const [pmeTab, setPmeTab] = useState<SchedulePmeTab>('preview')
  const [executionResult, setExecutionResult] =
    useState<ScheduleExecutionState | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [draftDueAfterHours, setDraftDueAfterHours] = useState<number | null>(
    null,
  )
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

  useEffect(() => {
    const nextDueAfterHours =
      snapshot?.config.dueAfterHours ?? loadState.status?.dueAfterHours ?? null
    if (nextDueAfterHours !== null) {
      setDraftDueAfterHours(nextDueAfterHours)
    }
  }, [loadState.requestKey, loadState.status?.dueAfterHours, snapshot])

  const plan = loadState.requestKey === refreshKey ? loadState.plan : null
  const status = loadState.requestKey === refreshKey ? loadState.status : null
  const error = loadState.requestKey === refreshKey ? loadState.error : null
  const loading = loadState.requestKey !== refreshKey
  const lastBackup =
    status?.lastSuccessfulBackupAt ??
    snapshot?.archiveStatus.lastSuccessfulBackupAt
  const latestAuditPath =
    executionResult?.result.auditPath ?? status?.auditPath ?? null
  const persistedDueAfterHours =
    snapshot?.config.dueAfterHours ?? status?.dueAfterHours ?? null
  const selectedDueAfterHours =
    draftDueAfterHours ?? persistedDueAfterHours ?? status?.dueAfterHours ?? 24
  const intervalDirty =
    persistedDueAfterHours !== null &&
    selectedDueAfterHours !== persistedDueAfterHours

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
  const primaryActionLabel = status
    ? schedulePrimaryActionLabel(status.installState, intervalDirty, t)
    : t('schedule.installFromCurrentSettings')
  const canPersistInterval =
    Boolean(snapshot?.config) && intervalDirty && busy === null
  const canApplySchedule =
    Boolean(snapshot?.config.initialized) &&
    Boolean(plan?.applySupported) &&
    Boolean(status?.applySupported) &&
    busy === null
  const scheduleAbsentWithoutFiles = status
    ? status.installState === 'not-installed' &&
      status.detectedFiles.length === 0
    : true
  const canRemoveSchedule =
    Boolean(plan?.applySupported) &&
    busy === null &&
    !scheduleAbsentWithoutFiles

  /**
   * Persists interval.
   *
   * Keeping this as a named declaration makes interval updates auditable before native schedule installation reuses the current plan/apply commands.
   */
  async function persistInterval() {
    if (!snapshot?.config) {
      throw new Error(t('schedule.initializeArchiveFirst'))
    }
    const nextSnapshot = await saveConfig({
      ...snapshot.config,
      dueAfterHours: selectedDueAfterHours,
    })
    setDraftDueAfterHours(nextSnapshot.config.dueAfterHours)
    return nextSnapshot
  }

  async function handleSaveInterval() {
    setBusy(t('schedule.saveInterval'))
    setActionError(null)
    setExecutionResult(null)

    try {
      await waitForNextPaint()
      await persistInterval()
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
   * Handles apply.
   *
   * Keeping this as a named declaration makes the Schedule surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleApply() {
    setBusy(primaryActionLabel)
    setActionError(null)
    setExecutionResult(null)

    try {
      await waitForNextPaint()
      let planToApply = plan!
      if (intervalDirty) {
        await persistInterval()
        planToApply = await backend.previewSchedule()
      }
      const result = await backend.applySchedule(planToApply)
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
    setBusy(t('schedule.removeSchedule'))
    setActionError(null)
    setExecutionResult(null)

    try {
      await waitForNextPaint()
      const result = await backend.removeSchedule(plan!)
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

  async function handleCopyValue(
    key: string,
    value: string,
    onFeedback = setCopyFeedback,
  ) {
    await copyReviewValue(value, {
      key,
      onFeedback,
    })
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
      <div className="schedule-overview-grid">
        <StatusCallout
          tone={scheduleInstallTone(status.installState)}
          eyebrow={t('schedule.platformBoundary')}
          title={t(platformLabelKey(status.platform))}
          body={t(platformSummaryKey(status.platform))}
        />
        <StatusCallout
          tone={scheduleInstallTone(status.installState)}
          eyebrow={t('schedule.statusBoundary')}
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
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">
            {t('schedule.configurationTitle')}
          </span>
          <span className="status-badge">{badge}</span>
        </div>
        <div className="panel-body">
          <div className="schedule-layout-grid">
            <div className="schedule-interval-panel">
              <div>
                <div className="summary-label">
                  {t('schedule.desiredInterval')}
                </div>
                <p className="dashboard-next-action">
                  {t('schedule.selectedIntervalValue', {
                    hours: selectedDueAfterHours,
                  })}
                </p>
              </div>
              <BackupIntervalSelector
                disabled={busy !== null}
                formatLabel={(hours) =>
                  t('schedule.intervalChipLabel', { hours })
                }
                value={selectedDueAfterHours}
                onChange={setDraftDueAfterHours}
              />
              {intervalDirty ? (
                <div className="warning-box schedule-inline-warning">
                  <div className="warning-icon">ℹ</div>
                  <div className="warning-text">
                    <strong>{t('schedule.intervalChangedTitle')}</strong>
                    <br />
                    {t('schedule.intervalChangedBody')}
                  </div>
                </div>
              ) : null}
              <div className="wizard-actions schedule-action-row">
                <button
                  className="btn-secondary"
                  disabled={!canPersistInterval}
                  type="button"
                  onClick={() => {
                    void handleSaveInterval()
                  }}
                >
                  {t('schedule.saveInterval')}
                </button>
                <button
                  className="btn-primary"
                  disabled={!canApplySchedule}
                  type="button"
                  onClick={() => {
                    void handleApply()
                  }}
                >
                  {primaryActionLabel}
                </button>
                <button
                  className="btn-secondary"
                  disabled={!canRemoveSchedule}
                  type="button"
                  onClick={() => {
                    void handleRemove()
                  }}
                >
                  {t('schedule.removeInstalledSchedule')}
                </button>
              </div>
              {!snapshot?.config.initialized ? (
                <p className="mono-support">
                  {t('schedule.initializeArchiveFirst')}
                </p>
              ) : null}
              {actionError ? (
                <p className="inline-error" role="alert">
                  {actionError}
                </p>
              ) : null}
            </div>

            <div className="schedule-config">
              <div className="config-row">
                <span className="config-label">
                  {t('schedule.installState')}
                </span>
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
                <span className="config-label">
                  {t('schedule.verification')}
                </span>
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
          </div>

          {status.warnings.map((warning) => (
            <div key={warning} className="warning-box">
              <div className="warning-icon">⚠</div>
              <div className="warning-text">{warning}</div>
            </div>
          ))}
        </div>
      </div>

      <SchedulePmePanel
        actionError={null}
        busy={busy}
        copyFeedback={copyFeedback}
        executionResult={executionResult}
        installDescription={installDescription}
        lastBackupLabel={
          lastBackup
            ? formatRelativeTime(lastBackup, language)
            : t('common.notAvailable')
        }
        latestAuditPath={latestAuditPath}
        onApply={() => {
          void handleApply()
        }}
        onCopyValue={(key, value) => handleCopyValue(key, value)}
        onOpenPath={async (path) => {
          await backend.openPathInFileManager(path)
        }}
        onRemove={() => {
          void handleRemove()
        }}
        plan={plan}
        pmeTab={pmeTab}
        setPmeTab={setPmeTab}
        snapshotInitialized={Boolean(snapshot?.config.initialized)}
        status={status}
        t={t}
      />
      {busy ? <BusyOverlay label={busy} /> : null}
    </section>
  )
}

function schedulePrimaryActionLabel(
  installState: string,
  intervalDirty: boolean,
  t: (key: string, vars?: Record<string, string | number>) => string,
) {
  if (intervalDirty) {
    return installState === 'not-installed'
      ? t('schedule.saveAndInstallSchedule')
      : t('schedule.saveAndUpdateSchedule')
  }
  if (installState === 'installed') return t('schedule.updateInstalledSchedule')
  if (installState === 'legacy-install-detected') {
    return t('schedule.installCanonicalSchedule')
  }
  return t('schedule.installFromCurrentSettings')
}
