/**
 * Restoring browsing history from a safety copy while the archive can't be
 * opened (`list_recovery_snapshots`, then `run_full_archive_restore`).
 *
 * Two passwords stay apart: the copy's (the archive's password when the copy
 * was made, `key`) and the archive's current one (`currentPassword`), which
 * the backend asks for when the archive is encrypted and closed, so the
 * restore keeps its encryption and password. Only when that current key
 * can't be opened at all does the person get the explicit choice to restore
 * under the copy's encryption instead, which starts Agent conversations
 * empty and keeps the current files aside as a copy.
 */
import { useMutation, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { archiveClient } from '@/lib/backend-client/archive'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import type { RecoverySnapshot } from '@/lib/types'
import { copyLabelKey } from '@/features/settings/copy-label'
import { snapshotFolderWords } from '@/features/settings/snapshot-folder-words'
import { restorable } from './classify'
import { AfterRepair, Panel, PasswordField, Unfold, Working } from './parts'

const NEEDS_CURRENT = 'archive-current-password-required'
const WRONG_PASSWORD = 'archive-password-wrong'
const COPY_PROTECTION_UNAVAILABLE = 'archive-copy-protection-unavailable'

export function CopyRestore() {
  const { t } = useI18n()
  const format = useFormat()
  const list = useQuery({
    queryKey: ['recovery-snapshots', 'access'],
    queryFn: archiveClient.listRecoverySnapshots,
    staleTime: 0,
    gcTime: 0,
  })
  const copies = (list.data ?? []).filter(restorable)
  const [chosenId, setChosenId] = useState<string | null>(null)
  const chosen = copies.find((copy) => copy.id === chosenId) ?? null
  const [key, setKey] = useState('')
  const [current, setCurrent] = useState('')
  const [askCurrent, setAskCurrent] = useState(false)
  const [keyLost, setKeyLost] = useState(false)
  const [agreed, setAgreed] = useState(false)

  const restore = useMutation({
    mutationFn: ({
      copy,
      copyEncryption,
    }: {
      copy: RecoverySnapshot
      copyEncryption: boolean
    }) =>
      archiveClient.runFullArchiveRestore(
        { snapshotPath: copy.path },
        {
          key: copy.encrypted ? key : null,
          currentPassword: askCurrent && !copyEncryption ? current : null,
          useCopyEncryption: copyEncryption,
        },
      ),
    onError: (error) => {
      if (commandErrorCode(error) === NEEDS_CURRENT) setAskCurrent(true)
    },
  })
  const code = restore.error ? commandErrorCode(restore.error) : null
  // The current key failed at least once: the explicit way out may be needed.
  const currentFailed = askCurrent && restore.isError && code !== NEEDS_CURRENT
  const ready =
    chosen !== null &&
    (!chosen.encrypted || key.length > 0) &&
    (!askCurrent || current.length > 0)

  if (restore.isSuccess) {
    return (
      <Panel
        title={t('archiveAccess.restore.title')}
        labelledBy="access-restore"
      >
        <AfterRepair text={t('archiveAccess.restore.done')} />
      </Panel>
    )
  }

  return (
    <Panel title={t('archiveAccess.restore.title')} labelledBy="access-restore">
      <p className="text-[13px] text-muted-foreground">
        {t('archiveAccess.restore.intro')}
      </p>
      {list.isPending ? (
        <div
          className="flex flex-col gap-2"
          aria-busy
          aria-label={t('archiveAccess.restore.loading')}
        >
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : list.isError ? (
        <p role="alert" className="text-[13px] text-destructive">
          {t('archiveAccess.restore.loadFailed', {
            message: describeError(list.error, 'list_recovery_snapshots'),
          })}
        </p>
      ) : copies.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">
          {t('archiveAccess.restore.none')}
        </p>
      ) : (
        <form
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            if (ready && !restore.isPending)
              restore.mutate({ copy: chosen, copyEncryption: false })
          }}
        >
          <RadioGroup
            value={chosenId ?? ''}
            onValueChange={(id) => {
              setChosenId(id)
              setKeyLost(false)
              setAgreed(false)
            }}
            aria-label={t('archiveAccess.restore.title')}
            className="max-h-64 gap-1 overflow-y-auto"
          >
            {copies.map((copy) => (
              <Label
                key={copy.id}
                htmlFor={`access-copy-${copy.id}`}
                className="flex items-center gap-3 rounded-lg border px-3 py-2.5 font-normal has-[[data-state=checked]]:border-foreground/30 has-[[data-state=checked]]:bg-muted"
              >
                <RadioGroupItem
                  id={`access-copy-${copy.id}`}
                  value={copy.id}
                  disabled={restore.isPending}
                />
                <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="font-medium">
                    {t(
                      `settingsStorage.restore.op.${copyLabelKey(copy.sourceOp)}`,
                    )}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {copy.createdAt
                      ? format.date(copy.createdAt, {
                          dateStyle: 'medium',
                          timeStyle: 'short',
                        })
                      : copy.label}
                    {' · '}
                    {format.bytes(copy.sizeBytes)}
                    {copy.unchecked &&
                      ` · ${t('settingsStorage.restore.unchecked')}`}
                  </span>
                </span>
              </Label>
            ))}
          </RadioGroup>

          <Unfold open={chosen?.encrypted ?? false}>
            <PasswordField
              id="access-copy-password"
              label={t('settingsStorage.restore.password')}
              hint={t('settingsStorage.restore.passwordHint')}
              value={key}
              onChange={setKey}
              disabled={restore.isPending}
            />
          </Unfold>
          <Unfold open={askCurrent}>
            <div className="flex flex-col gap-1.5">
              <p className="text-[13px] text-muted-foreground">
                {t('archiveAccess.restore.needsCurrent')}
              </p>
              <PasswordField
                id="access-current-password"
                label={t('archiveAccess.currentPassword')}
                hint={t('archiveAccess.currentPasswordHint')}
                value={current}
                onChange={setCurrent}
                invalid={code === WRONG_PASSWORD}
                disabled={restore.isPending}
              />
            </div>
          </Unfold>

          {restore.isPending && (
            <Working label={t('archiveAccess.restore.running')} />
          )}
          {restore.error && code !== NEEDS_CURRENT && (
            <p
              role="alert"
              className="text-[13px] [overflow-wrap:anywhere] text-destructive"
            >
              {code === WRONG_PASSWORD
                ? t(
                    askCurrent
                      ? 'archiveAccess.restore.wrongPassword'
                      : 'archiveAccess.restore.wrongCopyPassword',
                  )
                : code === COPY_PROTECTION_UNAVAILABLE
                  ? t('archiveAccess.restore.copyEncryption.unavailable')
                  : (snapshotFolderWords(t, restore.error) ??
                    t('archiveAccess.restore.failed', {
                      message: describeError(
                        restore.error,
                        'run_full_archive_restore',
                      ),
                    }))}
            </p>
          )}

          <Button
            type="submit"
            className="self-start"
            disabled={!ready || restore.isPending}
          >
            {restore.isPending && <Spinner />}
            {t('settingsStorage.restore.confirm')}
          </Button>

          {currentFailed && chosen && (
            <div className="flex flex-col gap-2 border-t pt-3">
              <Button
                type="button"
                variant="link"
                className="h-auto self-start p-0 text-[13px]"
                aria-expanded={keyLost}
                onClick={() => setKeyLost((open) => !open)}
              >
                {t('archiveAccess.restore.keyLost')}
              </Button>
              <Unfold open={keyLost}>
                <div className="flex flex-col gap-3 rounded-lg bg-muted p-3">
                  <h3 className="text-[13px] font-semibold">
                    {t('archiveAccess.restore.copyEncryption.title')}
                  </h3>
                  <p className="text-[13px] text-muted-foreground">
                    {t('archiveAccess.restore.copyEncryption.body')}
                  </p>
                  <div className="flex items-center gap-2">
                    <Checkbox
                      id="access-copy-encryption-agree"
                      checked={agreed}
                      disabled={restore.isPending}
                      onCheckedChange={(value) => setAgreed(value === true)}
                    />
                    <Label
                      htmlFor="access-copy-encryption-agree"
                      className="font-normal"
                    >
                      {t('archiveAccess.restore.copyEncryption.agree')}
                    </Label>
                  </div>
                  <Button
                    type="button"
                    variant="destructive"
                    className="self-start"
                    disabled={
                      !agreed ||
                      restore.isPending ||
                      (chosen.encrypted && key.length === 0)
                    }
                    onClick={() =>
                      restore.mutate({ copy: chosen, copyEncryption: true })
                    }
                  >
                    {t('archiveAccess.restore.copyEncryption.confirm')}
                  </Button>
                </div>
              </Unfold>
            </div>
          )}
        </form>
      )}
    </Panel>
  )
}
