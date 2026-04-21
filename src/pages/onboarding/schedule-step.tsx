/**
 * @file schedule-step.tsx
 * @description Renders the onboarding schedule preview step.
 * @module pages/onboarding
 */

import { LoadingState } from '../../components/primitives/loading-state'
import { useI18n } from '../../lib/i18n'
import type { SchedulePlan } from '../../lib/types'
import {
  dueAfterOptions,
  localizeScheduleManualStep,
  schedulePlatformLabel,
} from './shared'

export interface ScheduleStepProps {
  dueAfterHours: number
  onBack: () => void
  onContinue: () => void
  onSelectDueAfterHours: (hours: number) => void
  schedulePlan: SchedulePlan | null
  schedulePreviewError: string | null
  schedulePreviewLoading: boolean
}

export function ScheduleStep({
  dueAfterHours,
  onBack,
  onContinue,
  onSelectDueAfterHours,
  schedulePlan,
  schedulePreviewError,
  schedulePreviewLoading,
}: ScheduleStepProps) {
  const { t } = useI18n('onboarding')

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
          <div className="interval-chips">
            {dueAfterOptions.map((hours) => (
              <button
                className={`interval-chip ${dueAfterHours === hours ? 'active' : ''}`}
                key={hours}
                type="button"
                onClick={() => onSelectDueAfterHours(hours)}
              >
                {t('intervalChipLabel').replace('{hours}', String(hours))}
              </button>
            ))}
          </div>
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

      <div className="ob-actions">
        <button className="btn-secondary" type="button" onClick={onBack}>
          {t('backButton')}
        </button>
        <button className="btn-primary" type="button" onClick={onContinue}>
          {t('continueButton')}
        </button>
      </div>
    </div>
  )
}
