/**
 * Encrypt, change the password of, or decrypt the archive. Three steps:
 * choose the new password (or confirm decrypting), see what will happen
 * (`preview_rekey_archive`), then run it (`rekey_archive`).
 */
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { securityClient } from '@/lib/backend-client/security'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { RekeyPreview } from '@/lib/types'
import { emptySecret, secretProblem, type NewSecret } from './new-secret'
import { NewSecretFields } from './new-secret-fields'
import { Notice } from './notice'
import {
  ARCHIVE_PASSWORD_MIN_LENGTH,
  rekeyRequest,
  useRekey,
  type RekeyMode,
} from './use-rekey'

const knownWarnings = [
  'archive-locked',
  'new-key-required',
  'same-mode-rewrite',
] as const

export function RekeyDialog({
  mode,
  onClose,
}: {
  mode: RekeyMode | null
  onClose: () => void
}) {
  return (
    <Dialog open={mode !== null} onOpenChange={(open) => !open && onClose()}>
      {mode && <RekeyFlow key={mode} mode={mode} onClose={onClose} />}
    </Dialog>
  )
}

function RekeyFlow({
  mode,
  onClose,
}: {
  mode: RekeyMode
  onClose: () => void
}) {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const keyring = snapshot.keyringStatus
  const rekey = useRekey()
  const [secret, setSecret] = useState<NewSecret>(emptySecret)
  const [keep, setKeep] = useState(
    keyring.available && (mode === 'encrypt' || keyring.storedSecret),
  )
  const [preview, setPreview] = useState<RekeyPreview | null>(null)

  const needsPassword = mode !== 'decrypt'
  const passwordReady =
    !needsPassword ||
    secretProblem(secret, ARCHIVE_PASSWORD_MIN_LENGTH) === null

  const load = useMutation({
    mutationFn: () =>
      securityClient.previewRekey(rekeyRequest(mode, secret.value)),
    onSuccess: setPreview,
  })

  const run = useMutation({
    mutationFn: () => rekey(mode, secret.value, keep),
    onSuccess: ({ keychainFailed }) => {
      toast.success(t(`settings.security.rekey.done.${mode}`))
      if (keychainFailed) {
        toast.warning(t('settings.security.rekey.keychainFailed'))
      }
      onClose()
    },
  })

  const busy = load.isPending || run.isPending
  const error = load.error ?? run.error

  return (
    <DialogContent
      className="sm:max-w-md"
      showCloseButton={!run.isPending}
      onInteractOutside={(event) => run.isPending && event.preventDefault()}
      onEscapeKeyDown={(event) => run.isPending && event.preventDefault()}
    >
      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          if (busy) return
          if (!preview) {
            if (passwordReady) load.mutate()
          } else {
            run.mutate()
          }
        }}
      >
        <DialogHeader>
          <DialogTitle>
            {t(`settings.security.rekey.title.${mode}`)}
          </DialogTitle>
          <DialogDescription>
            {t(`settings.security.rekey.intro.${mode}`)}
          </DialogDescription>
        </DialogHeader>

        {!preview && needsPassword && (
          <>
            <NewSecretFields
              idPrefix="rekey"
              label={t('settings.security.rekey.newPassword')}
              secret={secret}
              minLength={ARCHIVE_PASSWORD_MIN_LENGTH}
              onChange={setSecret}
              disabled={busy}
            />
            {keyring.available && (
              <div className="flex items-center gap-2">
                <Checkbox
                  id="rekey-keychain"
                  checked={keep}
                  onCheckedChange={(value) => setKeep(value === true)}
                />
                <Label htmlFor="rekey-keychain" className="font-normal">
                  {t('settings.security.rekey.keepInKeychain')}
                </Label>
              </div>
            )}
            <Notice tone="warning">
              {t('settings.security.rekey.noReset')}
            </Notice>
          </>
        )}

        {preview && <PreviewSteps mode={mode} preview={preview} />}

        {run.isPending && (
          <div
            role="status"
            className="flex items-center gap-2 text-[13px] text-muted-foreground"
          >
            <Spinner />
            {t('settings.security.rekey.running')}
          </div>
        )}

        {error && (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {t(
              run.error
                ? 'settings.security.rekey.failed'
                : 'settings.security.rekey.previewFailed',
              { message: describeError(error) },
            )}
          </p>
        )}

        <DialogFooter>
          {preview ? (
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              onClick={() => setPreview(null)}
            >
              {t('common.back')}
            </Button>
          ) : (
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              onClick={onClose}
            >
              {t('common.cancel')}
            </Button>
          )}
          <Button
            type="submit"
            variant={preview && mode === 'decrypt' ? 'destructive' : 'default'}
            disabled={busy || !passwordReady}
          >
            {busy && <Spinner />}
            {preview
              ? t(`settings.security.rekey.run.${mode}`)
              : t('common.continue')}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}

function PreviewSteps({
  mode,
  preview,
}: {
  mode: RekeyMode
  preview: RekeyPreview
}) {
  const { t } = useI18n()
  const codes = preview.warningCodes ?? []
  const warnings = preview.warnings.map((prose, index) => {
    const code = codes[index]
    // "Enter a new password" can't happen here: the form already required one.
    if (code === 'new-key-required' || code === 'same-mode-rewrite') return null
    return knownWarnings.includes(code as (typeof knownWarnings)[number])
      ? t(`settings.security.rekey.warning.${code as 'archive-locked'}`)
      : prose
  })

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-sm font-medium">
        {t('settings.security.rekey.stepsTitle')}
      </h3>
      <ol className="flex list-decimal flex-col gap-2 pl-5 text-[13px] text-muted-foreground">
        <li>
          {t('settings.security.rekey.step.copy')}
          <span className="mt-0.5 block font-mono text-xs break-all text-foreground">
            {preview.snapshotPath}
          </span>
        </li>
        <li>
          {t(
            mode === 'decrypt'
              ? 'settings.security.rekey.step.writePlain'
              : 'settings.security.rekey.step.writeEncrypted',
          )}
        </li>
        <li>{t('settings.security.rekey.step.swap')}</li>
      </ol>
      {mode === 'decrypt' && (
        <Notice tone="warning">
          {t('settings.security.rekey.plainWarning')}
        </Notice>
      )}
      {warnings.map(
        (warning, index) =>
          warning && (
            <Notice key={index} tone="warning">
              {warning}
            </Notice>
          ),
      )}
    </div>
  )
}
