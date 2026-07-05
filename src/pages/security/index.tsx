/**
 * This module renders the Security route, where archive mode, keyring review, rekey preview, and lock-state recovery stay explicit.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `SecurityPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useEffect, useRef, useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { type ReviewCopyFeedback } from '../../components/review'
import { BusyOverlay } from '../../components/primitives/busy-overlay'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { useI18n } from '../../lib/i18n'
import { localizeSecurityWarning } from './helpers'
import {
  SecurityRekeyPanel,
  SecurityStatusPanel,
  SecurityUnlockPanel,
} from './panels'
import { useSecurityWorkflow } from './use-security-workflow'

/**
 * Renders the security route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Security expectations in the design docs.
 */
export function SecurityPage() {
  const { refreshAppData, refreshKey } = useShellData()
  const { language, t } = useI18n()
  const location = useLocation()
  const unlockInputRef = useRef<HTMLInputElement | null>(null)
  const [copyFeedback, setCopyFeedback] = useState<ReviewCopyFeedback | null>(
    null,
  )
  const {
    actionError,
    busy,
    handleExecuteRekey,
    handleLockArchive,
    handlePreviewRekey,
    handleUnlock,
    handleUnlockFromKeyring,
    notice,
    pageError,
    preview,
    rekeyConfirmText,
    rekeyKey,
    rekeyMode,
    saveRekeyKey,
    sessionKey,
    setPreview,
    setRekeyConfirmText,
    setRekeyKey,
    setRekeyMode,
    setSaveRekeyKey,
    setSessionKey,
    status,
  } = useSecurityWorkflow({ refreshAppData, refreshKey, t })
  const localizedWarnings = status
    ? status.warnings.map((warning) => localizeSecurityWarning(warning, t))
    : []

  useEffect(() => {
    if (
      location.hash !== '#unlock-archive' ||
      !status?.initialized ||
      !status.encrypted ||
      status.unlocked
    ) {
      return
    }

    const unlockInput = unlockInputRef.current
    if (!unlockInput) {
      return
    }

    unlockInput.focus()
    unlockInput.select()
    if (typeof unlockInput.scrollIntoView === 'function') {
      unlockInput.scrollIntoView({ block: 'center' })
    }
  }, [location.hash, status?.encrypted, status?.initialized, status?.unlocked])

  if (!status && pageError === null)
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="security-page"
      >
        <LoadingState label={t('security.loadingPosture')} />
      </div>
    )
  if (!status) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="security-page"
      >
        <EmptyState
          description={pageError!}
          eyebrow={t('navigation.securityLabel')}
          title={t('security.unavailableTitle')}
        />
      </div>
    )
  }

  if (!status.initialized) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="security-page"
      >
        <EmptyState
          action={
            <Link
              className="border-accent text-accent-text hover:bg-accent-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px]"
              to="/onboarding"
            >
              {t('security.initFirstAction')}
            </Link>
          }
          description={t('security.notInitializedBody')}
          eyebrow={t('navigation.securityLabel')}
          title={t('security.notInitializedTitle')}
        />
      </div>
    )
  }

  return (
    <div
      className="mx-auto flex w-full max-w-[1080px] flex-col gap-4 pt-7"
      data-testid="security-page"
    >
      {!status.keyringStatus.available ? (
        <StatusCallout
          tone="blocked"
          title={t('platform.keyringTitle')}
          body={t('platform.keyringBody')}
          actions={
            <Link
              className="border-border-default text-ink-muted hover:border-ink-muted hover:bg-hover rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px]"
              to="/settings"
            >
              {t('navigation.settingsLabel')}
            </Link>
          }
        />
      ) : null}

      <SecurityStatusPanel
        copyFeedback={copyFeedback}
        language={language}
        localizedWarnings={localizedWarnings}
        onOpenPath={(path) => {
          void backend.openPathInFileManager(path)
        }}
        setCopyFeedback={setCopyFeedback}
        status={status}
        t={t}
      />

      <SecurityUnlockPanel
        busy={busy}
        handleLockArchive={handleLockArchive}
        handleUnlock={handleUnlock}
        handleUnlockFromKeyring={handleUnlockFromKeyring}
        sessionKey={sessionKey}
        setSessionKey={setSessionKey}
        status={status}
        t={t}
        unlockInputRef={unlockInputRef}
      />

      <SecurityRekeyPanel
        actionError={actionError}
        busy={busy}
        handleExecuteRekey={handleExecuteRekey}
        handlePreviewRekey={handlePreviewRekey}
        localizedWarning={(warning) => localizeSecurityWarning(warning, t)}
        notice={notice}
        preview={preview}
        rekeyConfirmText={rekeyConfirmText}
        rekeyKey={rekeyKey}
        rekeyMode={rekeyMode}
        saveRekeyKey={saveRekeyKey}
        setPreview={setPreview}
        setRekeyConfirmText={setRekeyConfirmText}
        setRekeyKey={setRekeyKey}
        setRekeyMode={setRekeyMode}
        setSaveRekeyKey={setSaveRekeyKey}
        t={t}
      />
      {busy ? <BusyOverlay label={busy} /> : null}
    </div>
  )
}
