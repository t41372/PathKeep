/**
 * @file schedule-step.tsx
 * @description Renders the onboarding schedule preview step.
 * @module pages/onboarding
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
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

      <div className="mt-4">
        <PaperCard testId="onboarding-schedule-interval">
          <PaperCardHeader
            title={t('backupInterval')}
            right={<PaperCardBadge>{t('selectHours')}</PaperCardBadge>}
          />
          <PaperCardBody>
            <BackupIntervalSelector
              customInvalidMessage={t('intervalCustomInvalid')}
              customLabel={t('intervalCustomLabel')}
              customUnitLabel={t('intervalCustomUnit')}
              disabled={Boolean(busyAction)}
              formatLabel={(hours) =>
                t('intervalChipLabel').replace('{hours}', String(hours))
              }
              value={dueAfterHours}
              onChange={onSelectDueAfterHours}
            />
          </PaperCardBody>
        </PaperCard>
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
        <div className="mt-4">
          <PaperCard testId="onboarding-schedule-preview">
            <PaperCardHeader
              title={t('schedulePreview')}
              right={
                <PaperCardBadge>
                  {schedulePlatformLabel(schedulePlan.platform, t)}
                </PaperCardBadge>
              }
            />
            <PaperCardBody>
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
            </PaperCardBody>
          </PaperCard>
        </div>
      ) : null}

      <div className="mt-4">
        <StatusCallout
          tone="info"
          title={t('scheduleSkipHintTitle')}
          body={t('scheduleSkipHintBody')}
        />
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
