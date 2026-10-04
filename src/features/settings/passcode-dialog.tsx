/**
 * Set or change the app lock passcode (`set_app_lock_passcode`). Setting one
 * for the first time also turns app lock on, since that is why the user
 * asked for it.
 *
 * The optional hint is stored with the passcode and shown by "Forgot
 * passcode?" on the lock screen. The backend stores exactly the hint each
 * request carries, so changing the passcode starts from the saved hint;
 * leaving it out would erase it.
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { appClient } from '@/lib/backend-client/app'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import { emptySecret, secretProblem, type NewSecret } from './new-secret'
import { NewSecretFields } from './new-secret-fields'

/** The backend refuses anything shorter. */
const PASSCODE_MIN_LENGTH = 4

export function PasscodeDialog({
  mode,
  onClose,
}: {
  mode: 'set' | 'change' | null
  onClose: () => void
}) {
  return (
    <Dialog open={mode !== null} onOpenChange={(open) => !open && onClose()}>
      {mode && <PasscodeForm key={mode} mode={mode} onClose={onClose} />}
    </Dialog>
  )
}

function PasscodeForm({
  mode,
  onClose,
}: {
  mode: 'set' | 'change'
  onClose: () => void
}) {
  const { t } = useI18n()
  const client = useQueryClient()
  const savedHint = useSnapshot().appLockStatus.recoveryHint ?? ''
  const [secret, setSecret] = useState<NewSecret>(emptySecret)
  const [hint, setHint] = useState(mode === 'change' ? savedHint : '')
  const ready = secretProblem(secret, PASSCODE_MIN_LENGTH) === null

  const save = useMutation({
    mutationFn: async () => {
      await appClient.setAppLockPasscode({
        passcode: secret.value,
        recoveryHint: hint.trim() || null,
      })
      let snapshot = await appClient.getSnapshot()
      if (!snapshot.config.appLock.enabled) {
        const next = structuredClone(snapshot.config)
        next.appLock.enabled = true
        next.appLock.passcodeEnabled = true
        snapshot = await appClient.saveConfig(next, snapshot.config)
      }
      client.setQueryData(queryKeys.snapshot, snapshot)
    },
    onSuccess: () => {
      toast.success(
        t(
          mode === 'set'
            ? 'settings.security.passcode.savedOn'
            : 'settings.security.passcode.changed',
        ),
      )
      onClose()
    },
  })

  return (
    <DialogContent className="sm:max-w-sm">
      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          if (ready && !save.isPending) save.mutate()
        }}
      >
        <DialogHeader>
          <DialogTitle>
            {t(
              mode === 'set'
                ? 'settings.security.passcode.setTitle'
                : 'settings.security.passcode.changeTitle',
            )}
          </DialogTitle>
          <DialogDescription>
            {t('settings.security.passcode.dialogBody')}
          </DialogDescription>
        </DialogHeader>
        <NewSecretFields
          idPrefix="passcode"
          label={t('settings.security.passcode.label')}
          secret={secret}
          minLength={PASSCODE_MIN_LENGTH}
          onChange={setSecret}
          disabled={save.isPending}
        />
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="passcode-hint">
            {t('settings.security.passcode.hintLabel')}
          </Label>
          <Input
            id="passcode-hint"
            value={hint}
            autoComplete="off"
            disabled={save.isPending}
            onChange={(event) => setHint(event.target.value)}
          />
          <p className="text-[13px] text-muted-foreground">
            {t('settings.security.passcode.hintNote')}
          </p>
        </div>
        {save.error && (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {t('settings.security.passcode.saveFailed', {
              message: describeError(save.error, 'set_app_lock_passcode'),
            })}
          </p>
        )}
        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            disabled={save.isPending}
            onClick={onClose}
          >
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={!ready || save.isPending}>
            {save.isPending && <Spinner />}
            {t('settings.security.passcode.save')}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}
