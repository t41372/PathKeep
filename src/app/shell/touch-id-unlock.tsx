/**
 * "Unlock with Touch ID" on the app lock screen.
 *
 * Responsibilities: show the button only when the user turned Touch ID on in
 * Settings and the Mac has it; disable it with a note while Touch ID is
 * temporarily unavailable; ask the backend for the system prompt.
 *
 * Not responsible for: deciding whether Touch ID may unlock (the backend
 * refuses when Settings turned it off), or the passcode form.
 */
import { Fingerprint } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import type { AppLockStatus } from '@/lib/types'
import { useSession } from '../session'

/** Renders nothing unless Settings turned Touch ID on and this Mac has it. */
export function TouchIdUnlock({ status }: { status: AppLockStatus }) {
  const offered =
    status.biometricEnabled && status.biometricState !== 'unsupported'
  return offered ? (
    <TouchIdButton available={status.biometricAvailable} />
  ) : null
}

function TouchIdButton({ available }: { available: boolean }) {
  const { t } = useI18n()
  const session = useSession()
  const [busy, setBusy] = useState(false)
  const [failed, setFailed] = useState(false)

  const unlock = async () => {
    setBusy(true)
    setFailed(false)
    try {
      await session.unlockWithTouchId()
    } catch (error) {
      // Cancelling the prompt is a choice, not a failure.
      setFailed(commandErrorCode(error) !== 'biometric-canceled')
    } finally {
      setBusy(false)
    }
  }

  const note = !available
    ? t('shell.lock.touchIdUnavailable')
    : failed
      ? t('shell.lock.touchIdFailed')
      : null

  return (
    <div className="flex w-full flex-col items-center gap-1.5">
      <Button
        type="button"
        variant="outline"
        className="h-10 w-full bg-card"
        disabled={!available || busy}
        onClick={() => void unlock()}
      >
        {busy ? <Spinner /> : <Fingerprint />}
        {t('shell.lock.touchId')}
      </Button>
      {note && (
        <p role="status" className="text-[13px] text-muted-foreground">
          {note}
        </p>
      )}
    </div>
  )
}
