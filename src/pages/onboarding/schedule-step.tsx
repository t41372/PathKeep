/**
 * @file schedule-step.tsx
 * @description Renders the onboarding schedule preview step.
 * @module pages/onboarding
 */

import { LoadingState } from '../../components/primitives/loading-state'
import { BackupIntervalSelector } from '../../components/schedule/backup-interval-selector'
import { useI18n } from '../../lib/i18n'
import type { SchedulePlan, ScheduleStatus } from '../../lib/types'
import { localizeScheduleManualStep, schedulePlatformLabel } from './shared'

export interface ScheduleStepProps {
  dueAfterHours: number
  busyAction: string | null
  onBack: () => void
  onInstallSchedule: () => void
  onSelectDueAfterHours: (hours: number) => void
  onSkipSchedule: () => void
  schedulePlan: SchedulePlan | null
  schedulePreviewError: string | null
  schedulePreviewLoading: boolean
  scheduleStatus: ScheduleStatus | null
}

export function ScheduleStep({
  dueAfterHours,
  busyAction,
  onBack,
  onInstallSchedule,
  onSelectDueAfterHours,
  onSkipSchedule,
  schedulePlan,
  schedulePreviewError,
  schedulePreviewLoading,
  scheduleStatus,
}: ScheduleStepProps) {
  const { t } = useI18n('onboarding')
  const installDisabled =
    Boolean(busyAction) ||
    schedulePreviewLoading ||
    !schedulePlan ||
    !schedulePlan.applySupported

  return (
    <div className="ob-panel-container">
      <div className="ob-header">
        <div className="crosshair-mark">+</div>
        <h2 className="ob-title">{t('scheduleTitle')}</h2>
        <p className="ob-desc">{t('scheduleDesc')}</p>
      </div>

      <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
        <div className="panel-header">
          <span className="panel-title">{t('backupInterval')}</span>
          <span className="panel-action">{t('selectHours')}</span>
        </div>
        <div className="panel-body">
          <BackupIntervalSelector
            disabled={Boolean(busyAction)}
            formatLabel={(hours) =>
              t('intervalChipLabel').replace('{hours}', String(hours))
            }
            value={dueAfterHours}
            onChange={onSelectDueAfterHours}
          />
        </div>
      </div>

      {schedulePreviewLoading ? (
        <LoadingState label={t('previewingSchedule')} />
      ) : null}

      {schedulePreviewError ? (
        <p className="inline-error" role="alert">
          {schedulePreviewError}
        </p>
      ) : null}

      {schedulePlan ? (
        <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
          <div className="panel-header">
            <span className="panel-title">{t('schedulePreview')}</span>
            <span className="panel-action">
              {schedulePlatformLabel(schedulePlan.platform, t)}
            </span>
          </div>
          <div className="panel-body">
            {scheduleStatus ? (
              <div className="schedule-onboarding-status">
                <span className="summary-label">{t('scheduleStatus')}</span>
                <span className="status-badge">
                  {scheduleInstallStateLabel(scheduleStatus.installState, t)}
                </span>
              </div>
            ) : null}
            <div className="manual-steps">
              {schedulePlan.manualSteps.map((step, index) => (
                <div className="manual-step" key={`${index}-${step}`}>
                  <span className="step-num-inline">{index + 1}.</span>
                  <span>
                    {localizeScheduleManualStep(step, schedulePlan.label, t)}
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      ) : null}

      <div className="ob-info-box">
        <span className="info-icon">i</span>
        <span className="info-text">
          <strong>{t('scheduleSkipHintTitle')}</strong>
          <br />
          {t('scheduleSkipHintBody')}
        </span>
      </div>

      <div className="ob-actions">
        <button className="btn-secondary" type="button" onClick={onBack}>
          {t('backButton')}
        </button>
        <div className="wizard-actions">
          <button
            className="btn-secondary"
            type="button"
            onClick={onSkipSchedule}
          >
            {t('skipScheduleButton')}
          </button>
          <button
            className="btn-primary"
            disabled={installDisabled}
            type="button"
            onClick={onInstallSchedule}
          >
            {busyAction ?? t('installScheduleButton')}
          </button>
        </div>
      </div>
    </div>
  )
}

function scheduleInstallStateLabel(
  installState: string,
  t: (key: string) => string,
) {
  if (installState === 'installed') return t('scheduleInstalledBadge')
  if (installState === 'not-installed') return t('scheduleNotInstalledBadge')
  if (installState === 'manual-review') return t('scheduleManualReviewBadge')
  return t('scheduleAttentionBadge')
}
