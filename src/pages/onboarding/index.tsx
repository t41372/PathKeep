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
import { describeError } from '../../lib/errors'
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
import { AiStep } from './ai-step'
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
  // M-10: an IN-FLOW intent — "after this archive is set up, open AI settings". The AI step's CTA
  // records this and advances to the final review WITHOUT navigating away, so the entered/confirmed
  // master-password draft + step position are never discarded mid-onboarding. `handleFinish` honors
  // it as a post-initialize deep-link (AI settings need an initialized archive anyway).
  const [aiSetupRequested, setAiSetupRequested] = useState(false)

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
          setSchedulePreviewError(describeError(nextError, 'preview_schedule'))
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
    // `t` is intentionally NOT a dependency: the effect body never reads it, so
    // including it re-ran the schedule preview (two backend calls) on every
    // i18n context change / re-render that re-created the translator.
  }, [step, snapshot])

  if (loading && !snapshot) {
    return (
      <div
        className="mx-auto flex w-full max-w-[720px] flex-col pt-7"
        data-testid="onboarding-page"
      >
        <LoadingState label={t('loadingDecisions')} />
      </div>
    )
  }

  if (error && !snapshot) {
    return (
      <div
        className="mx-auto flex w-full max-w-[720px] flex-col pt-7"
        data-testid="onboarding-page"
      >
        <ErrorState description={error} title={t('errorTitle')} />
      </div>
    )
  }

  if (!snapshot) {
    return (
      <div
        className="mx-auto flex w-full max-w-[720px] flex-col pt-7"
        data-testid="onboarding-page"
      >
        <EmptyState
          description={t('emptyDescription')}
          eyebrow={t('emptyEyebrow')}
          title={t('emptyTitle')}
        />
      </div>
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

  // The AI step is purely optional and never enables AI or writes config; both "Set up AI in
  // Settings" and "Skip for now" advance to the final review. "Set up AI in Settings" does NOT
  // navigate away mid-flow (M-10) — that would unmount this page and discard the local `step` +
  // `securityDraft` (the confirmed master password). Instead it records an intent and advances to
  // review; `handleFinish` deep-links to AI settings AFTER the archive is initialized (which is the
  // correct order anyway — configuring an AI provider needs an initialized, unlocked archive).
  function handleAiSetUp() {
    setLocalError(null)
    setAiSetupRequested(true)
    setStep(6)
  }

  function handleAiContinue() {
    setLocalError(null)
    setAiSetupRequested(false)
    setStep(6)
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
          // Keyring writes can fail on locked / unavailable Secret Service
          // even after the mount-time probe said "available" (race, sudden
          // logout, etc). Translate the generic error into the keychain-
          // specific message so the user knows the archive itself is fine
          // and they just need to retry, uncheck the option, or use Settings.
          try {
            await backend.keyringStoreDatabaseKey(securityDraft.masterPassword)
          } catch {
            setLocalError(t('storeInKeyringFailed'))
            return
          }
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
      // M-10: honor the in-flow "set up AI" intent now that the archive is initialized + unlocked —
      // deep-link straight to AI settings instead of the dashboard. The master-password draft was
      // never discarded because the AI step never navigated away.
      void navigate(aiSetupRequested ? '/settings#settings-ai' : '/')
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
    <div
      className="mx-auto flex w-full max-w-[720px] flex-col gap-6 pt-7"
      data-testid="onboarding-page"
    >
      {step > 0 ? (
        <div className="flex w-full items-center justify-center">
          <div className="flex w-full max-w-[640px] items-center gap-1">
            {onboardingStepKeys.map((key, index) => (
              <div
                key={key}
                className="flex flex-1 items-center gap-1"
                style={{ display: 'contents' }}
              >
                <button
                  aria-current={index === step ? 'step' : undefined}
                  aria-label={`${t(key)}${index < step ? ' ✓' : ''}`}
                  className={`flex flex-1 flex-col items-center gap-1.5 text-center ${
                    index < step
                      ? 'cursor-pointer'
                      : index === step
                        ? ''
                        : 'opacity-60'
                  }`}
                  disabled={index > step}
                  type="button"
                  onClick={() => {
                    if (index < step) {
                      setStep(index)
                    }
                  }}
                >
                  <div
                    className={`grid h-7 w-7 place-items-center rounded-full border font-mono text-[10.5px] transition-colors ${
                      index < step
                        ? 'border-accent bg-accent text-white'
                        : index === step
                          ? 'border-accent text-accent-text bg-accent-soft'
                          : 'border-border-default text-ink-faint'
                    }`}
                  >
                    {index < step ? '✓' : index + 1}
                  </div>
                  <span
                    className={`font-sans text-[10.5px] tracking-wide ${
                      index === step ? 'text-ink font-medium' : 'text-ink-muted'
                    }`}
                  >
                    {t(key)}
                  </span>
                </button>
                {index < onboardingStepKeys.length - 1 ? (
                  <div
                    className={`mt-[-14px] h-px flex-1 ${
                      index < step ? 'bg-accent' : 'bg-border-light'
                    }`}
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
        <AiStep
          onBack={() => setStep(4)}
          onSetUpAi={handleAiSetUp}
          onSkip={handleAiContinue}
        />
      ) : null}

      {step === 6 ? (
        <ReadyStep
          appRoot={snapshot.directories.appRoot}
          archiveMode={currentConfig.archiveMode}
          busyAction={busyAction}
          dueAfterHours={currentConfig.dueAfterHours}
          localError={localError}
          scheduleSetupMode={scheduleSetupMode}
          selectedAccessIssueCount={selectedAccessIssueCount}
          selectedCount={selectedCount}
          onBack={() => setStep(5)}
          onFinish={() => {
            void handleFinish()
          }}
          onOpenFullDiskAccessSettings={handleOpenFullDiskAccessSettings}
        />
      ) : null}
    </div>
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

  return describeError(nextError, 'onboarding_finish')
}
