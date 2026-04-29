/**
 * @file index.tsx
 * @description Renders the onboarding route shell and keeps step ownership, validation, and setup-side effects in one route-level owner.
 * @module pages/onboarding
 *
 * ## 職責
 * - 處理 onboarding route 的 loading/error/empty gating。
 * - 持有 step state、config mutation、schedule preview、以及 finish validation/initialize flow。
 * - 組合 extracted onboarding step renderers與 stepper。
 *
 * ## 不負責
 * - 不在 route 內保留每一步的大段 JSX。
 * - 不把 onboarding draft state 塞回全域 context。
 * - 不改變既有 initialize / runBackup backend contract。
 *
 * ## 依賴關係
 * - 依賴 `useShellData()` 提供 snapshot、saveConfig、initializeArchive、runBackup。
 * - 依賴 extracted step modules 渲染各步畫面。
 *
 * ## 性能備注
 * - schedule preview 只在使用者進到 schedule step 時載入，避免 onboarding 首屏 fan-out。
 */

import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend-client'
import {
  formatBuildRevisionLabel,
  formatBuildVersionTitle,
} from '../../lib/build-info'
import { useI18n } from '../../lib/i18n'
import { estimateOnboardingStorage } from '../../lib/onboarding-estimates'
import {
  hasBrowserProfileAccessIssue,
  isBrowserProfileReadable,
  macosFullDiskAccessSettingsUrl,
} from '../../lib/platform-guidance'
import type { AppConfig, SchedulePlan, ScheduleStatus } from '../../lib/types'
import { BrowserDetectionStep } from './browser-detection-step'
import { ReadyStep } from './ready-step'
import { ScheduleStep } from './schedule-step'
import { SecurityStep } from './security-step'
import { onboardingStepKeys, type SecurityDraftState } from './shared'
import { StorageStep } from './storage-step'
import { WelcomeStep } from './welcome-step'

/**
 * Renders the onboarding route around extracted step renderers.
 *
 * The route keeps all workflow validation and side effects centralized so each
 * step renderer can stay presentational and easy to reason about.
 */
export function OnboardingPage() {
  const navigate = useNavigate()
  const {
    buildInfo,
    busyAction,
    error,
    loading,
    saveConfig,
    initializeArchive,
    runBackup,
    snapshot,
  } = useShellData()
  const { t } = useI18n('onboarding')
  const buildRevision = formatBuildRevisionLabel(buildInfo)
  const buildTitle = formatBuildVersionTitle(buildInfo)
  const [step, setStep] = useState(0)
  const [securityDraft, setSecurityDraft] = useState<SecurityDraftState>({
    confirmPassword: '',
    masterPassword: '',
    rememberKey: false,
  })
  const [localError, setLocalError] = useState<string | null>(null)
  const [schedulePlan, setSchedulePlan] = useState<SchedulePlan | null>(null)
  const [scheduleStatus, setScheduleStatus] = useState<ScheduleStatus | null>(
    null,
  )
  const [schedulePreviewLoading, setSchedulePreviewLoading] = useState(false)
  const [schedulePreviewError, setSchedulePreviewError] = useState<
    string | null
  >(null)
  const [scheduleSetupMode, setScheduleSetupMode] = useState<
    'install' | 'skip' | null
  >(null)

  useEffect(() => {
    if (step !== 4 || !snapshot) {
      return
    }

    let cancelled = false

    const loadSchedulePreview = async () => {
      setSchedulePreviewLoading(true)
      setSchedulePreviewError(null)
      try {
        const [plan, status] = await Promise.all([
          backend.previewSchedule(),
          backend.scheduleStatus(),
        ])
        if (!cancelled) {
          setSchedulePlan(plan)
          setScheduleStatus(status)
        }
      } catch (nextError) {
        if (!cancelled) {
          setSchedulePlan(null)
          setScheduleStatus(null)
          setSchedulePreviewError(
            nextError instanceof Error
              ? nextError.message
              : t('schedulePreviewFallbackError'),
          )
        }
      } finally {
        if (!cancelled) {
          setSchedulePreviewLoading(false)
        }
      }
    }

    void loadSchedulePreview()

    return () => {
      cancelled = true
    }
  }, [step, snapshot, t])

  if (loading && !snapshot) {
    return (
      <section
        className="page-shell onboarding-page"
        data-testid="onboarding-page"
      >
        <LoadingState label={t('loadingDecisions')} />
      </section>
    )
  }

  if (error && !snapshot) {
    return (
      <section
        className="page-shell onboarding-page"
        data-testid="onboarding-page"
      >
        <ErrorState description={error} title={t('errorTitle')} />
      </section>
    )
  }

  if (!snapshot) {
    return (
      <section
        className="page-shell onboarding-page"
        data-testid="onboarding-page"
      >
        <EmptyState
          description={t('emptyDescription')}
          eyebrow={t('emptyEyebrow')}
          title={t('emptyTitle')}
        />
      </section>
    )
  }

  const currentConfig = snapshot.config
  const selectedProfiles = currentConfig.selectedProfileIds
    .map((id) =>
      snapshot.browserProfiles.find((profile) => profile.profileId === id),
    )
    .filter((profile): profile is NonNullable<typeof profile> =>
      Boolean(profile),
    )
  const selectedCount = selectedProfiles.filter(isBrowserProfileReadable).length
  const selectedAccessIssueCount = selectedProfiles.filter(
    hasBrowserProfileAccessIssue,
  ).length
  const storageEstimate = estimateOnboardingStorage(
    snapshot.browserProfiles,
    currentConfig.selectedProfileIds,
  )

  async function updateConfig(
    updater: (config: typeof currentConfig) => typeof currentConfig,
  ) {
    setLocalError(null)
    await saveConfig(updater(currentConfig))
  }

  function handleSecurityCardClick(
    mode: AppConfig['archiveMode'],
    target: EventTarget | null,
  ) {
    const element = target instanceof HTMLElement ? target : null
    if (element?.closest('button, input, select, textarea, a, label')) {
      return
    }
    void updateConfig((config) => ({ ...config, archiveMode: mode }))
  }

  function handleBrowsersContinue() {
    setLocalError(null)
    if (selectedCount === 0) {
      setLocalError(
        selectedAccessIssueCount > 0
          ? t('errorSelectedProfilesNeedAccess')
          : t('errorSelectProfile'),
      )
      return
    }
    setStep(2)
  }

  async function handleOpenFullDiskAccessSettings() {
    setLocalError(null)
    try {
      await backend.openExternalUrl(macosFullDiskAccessSettingsUrl)
    } catch {
      setLocalError(t('errorOpenFullDiskAccessSettings'))
    }
  }

  function updateSecurityDraft(next: Partial<SecurityDraftState>) {
    setLocalError(null)
    setSecurityDraft((current) => ({
      ...current,
      ...next,
    }))
  }

  function handleSecurityContinue() {
    setLocalError(null)
    if (currentConfig.archiveMode !== 'Encrypted') {
      setStep(4)
      return
    }
    if (!securityDraft.masterPassword.trim()) {
      setLocalError(t('errorNeedPassword'))
      return
    }
    if (securityDraft.masterPassword !== securityDraft.confirmPassword) {
      setLocalError(t('errorPasswordMismatch'))
      return
    }
    setStep(4)
  }

  function handleScheduleInstallIntent() {
    setLocalError(null)
    setScheduleSetupMode('install')
    setStep(5)
  }

  function handleScheduleSkip() {
    setLocalError(null)
    setScheduleSetupMode('skip')
    setStep(5)
  }

  async function handleFinish() {
    setLocalError(null)
    if (selectedCount === 0) {
      setLocalError(
        selectedAccessIssueCount > 0
          ? t('errorSelectedProfilesNeedAccess')
          : t('errorSelectProfile'),
      )
      return
    }
    const encrypted = currentConfig.archiveMode === 'Encrypted'
    if (encrypted) {
      if (!securityDraft.masterPassword.trim()) {
        setLocalError(t('errorNeedPassword'))
        return
      }
      if (securityDraft.masterPassword !== securityDraft.confirmPassword) {
        setLocalError(t('errorPasswordMismatch'))
        return
      }
    }
    try {
      if (!currentConfig.initialized) {
        await initializeArchive(
          currentConfig,
          encrypted ? securityDraft.masterPassword : null,
        )
        if (encrypted && securityDraft.rememberKey) {
          await backend.keyringStoreDatabaseKey(securityDraft.masterPassword)
        }
      }
      if (scheduleSetupMode === 'install') {
        try {
          const planToApply = schedulePlan ?? (await backend.previewSchedule())
          await backend.applySchedule(planToApply)
        } catch {
          setLocalError(t('errorScheduleInstallFailed'))
          return
        }
      }
      await runBackup()
      void navigate('/')
    } catch (nextError) {
      setLocalError(formatOnboardingError(nextError, t))
    }
  }

  function handleToggleProfile(profileId: string) {
    const selected = currentConfig.selectedProfileIds.includes(profileId)
    const nextSelected = selected
      ? currentConfig.selectedProfileIds.filter((value) => value !== profileId)
      : [...currentConfig.selectedProfileIds, profileId]
    void updateConfig((config) => ({
      ...config,
      selectedProfileIds: nextSelected,
    }))
  }

  return (
    <section data-testid="onboarding-page">
      {step > 0 ? (
        <div className="onboarding-stepper">
          <div className="stepper-track">
            {onboardingStepKeys.map((key, index) => (
              <div key={key} style={{ display: 'contents' }}>
                <button
                  aria-current={index === step ? 'step' : undefined}
                  aria-label={`${t(key)}${index < step ? ' ✓' : ''}`}
                  className={`stepper-step ${index < step ? 'completed' : ''} ${index === step ? 'active' : ''} ${index < step ? 'clickable' : ''}`}
                  disabled={index > step}
                  type="button"
                  onClick={() => {
                    if (index < step) {
                      setStep(index)
                    }
                  }}
                >
                  <div className="stepper-dot">
                    <span className="stepper-check">✓</span>
                    <span className="stepper-num">{index + 1}</span>
                  </div>
                  <span className="stepper-label">{t(key)}</span>
                </button>
                {index < onboardingStepKeys.length - 1 ? (
                  <div
                    className={`stepper-line ${index < step ? 'completed' : ''} ${index === step - 1 ? 'active' : ''}`}
                  />
                ) : null}
              </div>
            ))}
          </div>
        </div>
      ) : null}

      {step === 0 ? (
        <WelcomeStep
          buildInfo={buildInfo}
          buildRevision={buildRevision}
          buildTitle={buildTitle}
          onBegin={() => setStep(1)}
        />
      ) : null}

      {step === 1 ? (
        <BrowserDetectionStep
          browserProfiles={snapshot.browserProfiles}
          busyAction={busyAction}
          localError={localError}
          selectedAccessIssueCount={selectedAccessIssueCount}
          selectedCount={selectedCount}
          selectedProfileIds={currentConfig.selectedProfileIds}
          onBack={() => setStep(0)}
          onContinue={handleBrowsersContinue}
          onOpenFullDiskAccessSettings={handleOpenFullDiskAccessSettings}
          onToggleProfile={handleToggleProfile}
        />
      ) : null}

      {step === 2 ? (
        <StorageStep
          appRoot={snapshot.directories.appRoot}
          storageEstimate={storageEstimate}
          onBack={() => setStep(1)}
          onContinue={() => setStep(3)}
        />
      ) : null}

      {step === 3 ? (
        <SecurityStep
          archiveMode={currentConfig.archiveMode}
          busyAction={busyAction}
          localError={localError}
          securityDraft={securityDraft}
          onBack={() => setStep(2)}
          onContinue={handleSecurityContinue}
          onSecurityCardClick={handleSecurityCardClick}
          onSelectArchiveMode={(mode) => {
            void updateConfig((config) => ({ ...config, archiveMode: mode }))
          }}
          onUpdateSecurityDraft={updateSecurityDraft}
        />
      ) : null}

      {step === 4 ? (
        <ScheduleStep
          busyAction={busyAction}
          dueAfterHours={currentConfig.dueAfterHours}
          schedulePlan={schedulePlan}
          schedulePreviewError={schedulePreviewError}
          schedulePreviewLoading={schedulePreviewLoading}
          scheduleStatus={scheduleStatus}
          onBack={() => setStep(3)}
          onInstallSchedule={handleScheduleInstallIntent}
          onSelectDueAfterHours={(hours) => {
            setScheduleSetupMode(null)
            void updateConfig((config) => ({ ...config, dueAfterHours: hours }))
          }}
          onSkipSchedule={handleScheduleSkip}
        />
      ) : null}

      {step === 5 ? (
        <ReadyStep
          appRoot={snapshot.directories.appRoot}
          archiveMode={currentConfig.archiveMode}
          busyAction={busyAction}
          dueAfterHours={currentConfig.dueAfterHours}
          localError={localError}
          scheduleSetupMode={scheduleSetupMode}
          selectedAccessIssueCount={selectedAccessIssueCount}
          selectedCount={selectedCount}
          onBack={() => setStep(4)}
          onFinish={() => {
            void handleFinish()
          }}
          onOpenFullDiskAccessSettings={handleOpenFullDiskAccessSettings}
        />
      ) : null}
    </section>
  )
}

function formatOnboardingError(
  nextError: unknown,
  t: (key: string, params?: Record<string, string | number>) => string,
) {
  if (
    nextError instanceof Error &&
    (nextError.message.includes('Full Disk Access') ||
      nextError.message.includes('完全磁盘访问权限') ||
      nextError.message.includes('完整磁碟取用權') ||
      nextError.message.includes('Safari History.db'))
  ) {
    return t('errorSafariNeedsFullDiskAccess')
  }

  return t('errorFinishFailed')
}
