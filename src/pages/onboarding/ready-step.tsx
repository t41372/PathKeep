/**
 * @file ready-step.tsx
 * @description Renders the onboarding final review step before archive initialization and first backup.
 * @module pages/onboarding
 */

import { useI18n } from '../../lib/i18n'

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

  return (
    <div className="ob-panel-container">
      <div className="ob-header">
        <div className="crosshair-mark">+</div>
        <h2 className="ob-title">{t('readyTitle')}</h2>
        <p className="ob-desc">{t('readyDesc')}</p>
      </div>

      <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
        <div className="panel-header">
          <span className="panel-title">{t('configSummary')}</span>
          <span className="panel-action">{t('reviewBeforeInit')}</span>
        </div>
        <div className="panel-body">
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
                  : scheduleSetupMode === 'install'
                    ? t('configScheduleInstallValue').replace(
                        '{hours}',
                        String(dueAfterHours),
                      )
                    : t('configScheduleValue').replace(
                        '{hours}',
                        String(dueAfterHours),
                      )}
              </span>
            </div>
          </div>
        </div>
      </div>

      {scheduleSetupMode === 'skip' ? (
        <div className="ob-info-box">
          <span className="info-icon">i</span>
          <span className="info-text">
            <strong>{t('scheduleSkippedTitle')}</strong>
            <br />
            {t('scheduleSkippedBody')}
          </span>
        </div>
      ) : null}

      <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
        <div className="panel-header">
          <span className="panel-title">{t('initSteps')}</span>
          <span className="panel-action">{t('whatHappensNext')}</span>
        </div>
        <div className="panel-body">
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
        </div>
      </div>

      {selectedAccessIssueCount > 0 ? (
        <div className="ob-info-box ob-info-box--warning">
          <span className="info-icon">!</span>
          <span className="info-text">
            <strong>{t('readyAccessWarningTitle')}</strong>
            <br />
            {t('readyAccessWarningBody')}
          </span>
          <button
            className="btn-secondary"
            type="button"
            onClick={onOpenFullDiskAccessSettings}
          >
            {t('openFullDiskAccessSettings')}
          </button>
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
