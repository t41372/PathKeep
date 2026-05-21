/**
 * @file ready-step.tsx
 * @description Renders the onboarding final review step before archive initialization and first backup.
 * @module pages/onboarding
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n'
import { backupIntervalHoursToMinutes } from '../../lib/schedule-options'

export interface ReadyStepProps {
  appRoot: string
  archiveMode: 'Encrypted' | 'Plaintext'
  busyAction: string | null
  dueAfterHours: number
  localError: string | null
  onBack: () => void
  onFinish: () => void
  onOpenFullDiskAccessSettings: () => void
  scheduleSetupMode: 'install' | 'skip' | null
  selectedAccessIssueCount: number
  selectedCount: number
}

export function ReadyStep({
  appRoot,
  archiveMode,
  busyAction,
  dueAfterHours,
  localError,
  onBack,
  onFinish,
  onOpenFullDiskAccessSettings,
  scheduleSetupMode,
  selectedAccessIssueCount,
  selectedCount,
}: ReadyStepProps) {
  const { t, ns } = useI18n('onboarding')
  const commonT = ns('common')
  const scheduleValue = formatOnboardingScheduleValue({
    dueAfterHours,
    installing: scheduleSetupMode === 'install',
    t,
  })

  return (
    <div className="ob-panel-container">
      <div className="ob-header">
        <div className="crosshair-mark">+</div>
        <h2 className="ob-title">{t('readyTitle')}</h2>
        <p className="ob-desc">{t('readyDesc')}</p>
      </div>

      <div className="mt-4">
        <PaperCard testId="onboarding-ready-config-summary">
          <PaperCardHeader
            title={t('configSummary')}
            right={<PaperCardBadge>{t('reviewBeforeInit')}</PaperCardBadge>}
          />
          <PaperCardBody>
            <div className="summary-config">
              <div className="config-row">
                <span className="config-label">{t('configProfiles')}</span>
                <span className="config-value">
                  {t('configProfilesValue').replace(
                    '{count}',
                    String(selectedCount),
                  )}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">{t('configStorage')}</span>
                <span className="config-value">{appRoot}</span>
              </div>
              <div className="config-row">
                <span className="config-label">{t('configEncryption')}</span>
                <span className="config-value">
                  {archiveMode === 'Encrypted'
                    ? commonT('modeEncrypted')
                    : commonT('modePlaintext')}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">{t('configSchedule')}</span>
                <span className="config-value">
                  {scheduleSetupMode === 'skip'
                    ? t('configScheduleSkippedValue')
                    : scheduleValue}
                </span>
              </div>
            </div>
          </PaperCardBody>
        </PaperCard>
      </div>

      {scheduleSetupMode === 'skip' ? (
        <div className="mt-4">
          <StatusCallout
            tone="info"
            title={t('scheduleSkippedTitle')}
            body={t('scheduleSkippedBody')}
          />
        </div>
      ) : null}

      <div className="mt-4">
        <PaperCard testId="onboarding-ready-init-steps">
          <PaperCardHeader
            title={t('initSteps')}
            right={<PaperCardBadge>{t('whatHappensNext')}</PaperCardBadge>}
          />
          <PaperCardBody>
            <div className="init-steps">
              <div className="init-step">
                <span className="init-num">1.</span>
                <div className="init-info">
                  <span className="init-action">{t('initStep1Action')}</span>
                  <span className="init-detail">
                    {archiveMode === 'Encrypted'
                      ? t('initStep1DetailEncrypted')
                      : t('initStep1DetailPlaintext')}
                  </span>
                </div>
              </div>
              <div className="init-step">
                <span className="init-num">2.</span>
                <div className="init-info">
                  <span className="init-action">{t('initStep2Action')}</span>
                  <span className="init-detail">{t('initStep2Detail')}</span>
                </div>
              </div>
              <div className="init-step">
                <span className="init-num">3.</span>
                <div className="init-info">
                  <span className="init-action">{t('initStep3Action')}</span>
                  <span className="init-detail">
                    {t('initStep3Detail')
                      .replace('{count}', String(selectedCount))
                      .replace('{plural}', selectedCount !== 1 ? 's' : '')}
                  </span>
                </div>
              </div>
            </div>
          </PaperCardBody>
        </PaperCard>
      </div>

      {selectedAccessIssueCount > 0 ? (
        <div className="mt-4">
          <StatusCallout
            tone="warning"
            title={t('readyAccessWarningTitle')}
            body={t('readyAccessWarningBody')}
            actions={
              <button
                className="btn-secondary"
                type="button"
                onClick={onOpenFullDiskAccessSettings}
              >
                {t('openFullDiskAccessSettings')}
              </button>
            }
          />
        </div>
      ) : null}

      {localError ? (
        <p className="inline-error" role="alert">
          {localError}
        </p>
      ) : null}

      <div className="ob-actions">
        <button className="btn-secondary" type="button" onClick={onBack}>
          {t('backButton')}
        </button>
        <button
          className="btn-primary btn-lg"
          type="button"
          disabled={busyAction !== null}
          onClick={onFinish}
        >
          {busyAction ?? t('initButton')}
        </button>
      </div>
    </div>
  )
}

function formatOnboardingScheduleValue({
  dueAfterHours,
  installing,
  t,
}: {
  dueAfterHours: number
  installing: boolean
  t: (key: string) => string
}): string {
  const minutes = backupIntervalHoursToMinutes(dueAfterHours)
  if (minutes % 60 === 0) {
    return t(
      installing ? 'configScheduleInstallValue' : 'configScheduleValue',
    ).replace('{hours}', String(minutes / 60))
  }
  return t(
    installing
      ? 'configScheduleInstallValueMinutes'
      : 'configScheduleValueMinutes',
  ).replace('{minutes}', String(minutes))
}
