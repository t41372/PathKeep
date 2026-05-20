/**
 * This module renders the standalone lock screen that protects the shell when App Lock is enabled or an idle timeout expires.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `LockPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { BrandMark } from '../../components/brand-mark'
import {
  PaperCard,
  PaperCardBody,
} from '@/components/cards'
import {
  copyReviewValue,
  ReviewPathActionRow,
  type ReviewCopyFeedback,
} from '../../components/review'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import {
  formatBuildVersionLabel,
  formatBuildVersionTitle,
} from '../../lib/build-info'
import { useI18n } from '../../lib/i18n'
import { cn } from '../../lib/cn'

/**
 * Explains how lock reason label works.
 *
 * Keeping this as a named declaration makes the Lock surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function lockReasonLabel(
  reason: string | null | undefined,
  t: (key: string, params?: Record<string, string | number>) => string,
) {
  switch (reason) {
    case 'idle-timeout':
      return t('shell.lockReasonIdleTimeout')
    case 'startup':
      return t('shell.lockReasonStartup')
    default:
      return t('shell.lockReasonManual')
  }
}

/**
 * Renders the lock route.
 *
 * Paper aesthetic: serif title + JetBrains Mono kicker + paper card unlock
 * form, centered on the page background. The form is intentionally minimal
 * — no chrome around the page, just a single PaperCard the user touches.
 */
export function LockPage() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const { appLockStatus, buildInfo, error, unlockAppSession } = useShellData()
  const { t } = useI18n()
  const buildLabel = formatBuildVersionLabel(buildInfo)
  const buildTitle = formatBuildVersionTitle(buildInfo)
  const [passcode, setPasscode] = useState('')
  const [unlocking, setUnlocking] = useState(false)
  const [copyFeedback, setCopyFeedback] = useState<ReviewCopyFeedback | null>(
    null,
  )

  if (!appLockStatus) {
    return (
      <div
        className="bg-page text-ink min-h-screen w-full"
        data-testid="lock-page"
      >
        <div className="mx-auto flex w-full max-w-[520px] flex-col items-center justify-center px-6 py-12">
          <LoadingState label={t('common.loading')} />
        </div>
      </div>
    )
  }

  const nextPath = searchParams.get('next')?.trim() || '/'
  const reason = lockReasonLabel(appLockStatus.lockReason, t)
  const canTryBiometric = appLockStatus.biometricEnabled
  const touchIdState =
    appLockStatus.biometricState === 'touch-id-available' ||
    appLockStatus.biometricState === 'touch-id-unavailable'

  /**
   * Handles unlock.
   *
   * Keeping this as a named declaration makes the Lock surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleUnlock(useBiometric = false) {
    setUnlocking(true)
    try {
      await unlockAppSession({
        passcode: useBiometric ? null : passcode,
        useBiometric,
      })
      setPasscode('')
      void navigate(nextPath, { replace: true })
    } finally {
      setUnlocking(false)
    }
  }

  return (
    <div
      className="bg-page text-ink min-h-screen w-full"
      data-testid="lock-page"
    >
      <div className="mx-auto flex w-full max-w-[520px] flex-col gap-5 px-6 py-12">
        <div className="flex flex-col items-center gap-3 pb-2">
          <div className="flex items-center gap-3">
            <div aria-hidden className="h-9 w-9">
              <BrandMark alt="" />
            </div>
            <div className="flex flex-col">
              <span className="text-ink font-serif text-[16px] font-medium tracking-[-0.01em]">
                PathKeep
              </span>
              <span
                className="text-ink-faint font-mono text-[10.5px] tracking-[0.06em]"
                title={buildTitle ?? undefined}
              >
                {buildLabel ?? t('common.notAvailable')}
              </span>
            </div>
          </div>
          <span className="text-ink-faint mt-3 font-mono text-[10px] tracking-[0.18em] uppercase">
            {t('shell.lockEyebrow')}
          </span>
          <h1 className="text-ink m-0 font-serif text-[24px] leading-tight font-medium tracking-[-0.01em]">
            {t('shell.lockTitle')}
          </h1>
          <p className="text-ink-muted m-0 max-w-[440px] text-center font-serif text-[13.5px] leading-[1.55] italic">
            {t('shell.lockDescription')}
          </p>
        </div>

        <PaperCard>
          <PaperCardBody>
            <div className="border-border-light flex flex-col gap-2 border-b pb-3">
              <div className="flex items-center justify-between gap-3">
                <span className="text-ink-faint font-mono text-[10px] tracking-[0.08em] uppercase">
                  {t('shell.lockReason')}
                </span>
                <span className="text-ink-muted font-mono text-[11px]">
                  {reason}
                </span>
              </div>
              <ReviewPathActionRow
                copyFeedback={copyFeedback}
                copyKey="lock:config-path"
                copyLabel={t('common.copyAction')}
                errorMessage={t('audit.copyFailed')}
                label={t('shell.lockConfigPath')}
                onCopy={(key, value) => {
                  void copyReviewValue(value, {
                    key,
                    onFeedback: setCopyFeedback,
                  })
                }}
                onOpenPath={(path) => {
                  void backend.openPathInFileManager(path)
                }}
                openPathLabel={t('shell.lockRecoveryAction')}
                successMessage={t('common.copiedNotice')}
                value={appLockStatus.configPath}
              />
              <div className="flex items-center justify-between gap-3">
                <span className="text-ink-faint font-mono text-[10px] tracking-[0.08em] uppercase">
                  {t('shell.lastUnlockedAt')}
                </span>
                <span className="text-ink-muted font-mono text-[11px]">
                  {appLockStatus.lastUnlockedAt ?? t('common.notAvailable')}
                </span>
              </div>
            </div>

            {error ? (
              <div className="pt-3">
                <StatusCallout
                  tone="danger"
                  title={t('shell.unlockAppFailed')}
                  body={error}
                />
              </div>
            ) : null}

            {appLockStatus.warnings.map((warning) => (
              <div key={warning} className="pt-3">
                <StatusCallout
                  tone="warning"
                  title={t('common.warning')}
                  body={warning}
                />
              </div>
            ))}

            <form
              className="flex flex-col gap-3 pt-4"
              onSubmit={(event) => {
                event.preventDefault()
                void handleUnlock(false)
              }}
            >
              <label className="flex flex-col gap-1.5">
                <span className="text-ink-faint font-mono text-[10px] tracking-[0.08em] uppercase">
                  {t('shell.lockPasscodeLabel')}
                </span>
                <input
                  aria-label={t('shell.lockPasscodeLabel')}
                  autoComplete="current-password"
                  className={cn(
                    'border-border-default rounded-paper bg-paper text-ink w-full border px-3 py-2 font-mono text-[12px] tracking-[0.04em]',
                    'focus:border-accent focus:outline-none',
                    'disabled:opacity-60',
                  )}
                  disabled={unlocking}
                  placeholder={t('shell.lockPasscodePlaceholder')}
                  type="password"
                  value={passcode}
                  onChange={(event) => setPasscode(event.target.value)}
                />
              </label>

              <div className="flex flex-wrap items-center gap-2">
                <button
                  className={cn(
                    'border-accent text-accent-text rounded-paper border px-4 py-1.5 font-sans text-[12.5px] font-medium transition-colors',
                    'hover:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-60',
                  )}
                  type="submit"
                  disabled={unlocking || !passcode.trim()}
                >
                  {unlocking ? t('shell.unlockingApp') : t('shell.unlockApp')}
                </button>
                {canTryBiometric ? (
                  <button
                    className={cn(
                      'border-border-default text-ink-muted rounded-paper border px-4 py-1.5 font-sans text-[12.5px] transition-colors',
                      'hover:border-ink-muted hover:bg-hover disabled:cursor-not-allowed disabled:opacity-60',
                    )}
                    type="button"
                    disabled={unlocking || !appLockStatus.biometricAvailable}
                    onClick={() => {
                      void handleUnlock(true)
                    }}
                  >
                    {touchIdState
                      ? t('shell.unlockWithTouchId')
                      : t('shell.unlockWithBiometric')}
                  </button>
                ) : null}
              </div>
            </form>

            {canTryBiometric && !appLockStatus.biometricAvailable ? (
              <p className="text-ink-faint mt-3 font-mono text-[10.5px]">
                {touchIdState
                  ? t('shell.unlockTouchIdUnavailable')
                  : t('shell.unlockBiometricUnavailable')}
              </p>
            ) : null}

            <div className="pt-4">
              <StatusCallout
                tone="info"
                title={t('shell.lockRecoveryTitle')}
                body={
                  appLockStatus.recoveryHint
                    ? t('shell.lockRecoveryHintBody', {
                        hint: appLockStatus.recoveryHint,
                      })
                    : t('shell.lockRecoveryBody')
                }
              />
            </div>

            {appLockStatus.degradationNotes.length ? (
              <div className="border-border-light mt-4 flex flex-col gap-1 border-t pt-3">
                {appLockStatus.degradationNotes.map((note) => (
                  <p
                    key={note}
                    className="text-ink-faint m-0 font-mono text-[10.5px]"
                  >
                    {note}
                  </p>
                ))}
              </div>
            ) : null}
          </PaperCardBody>
        </PaperCard>
      </div>
    </div>
  )
}
