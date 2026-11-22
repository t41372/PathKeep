/**
 * Going back to PathKeep 0.5.0 (`preview_archive_rollback`, then
 * `prepare_archive_rollback`). PathKeep 0.5.0 can't open an archive this
 * version changed; preparing turns it back into a form 0.5.0 opens without
 * copying the archive: browsing history stays where it is, and what 0.5.0
 * can't use (Agent conversations, the copies and move files this version
 * made) moves by rename into a package next to the data folder.
 *
 * The preview says what moves, what 0.5.0 will and won't see (it doesn't see
 * the conversations, its "Delete all data" doesn't reach them, their password
 * stays the one they have now) and what happens when this version opens the
 * folder again. Preparing turns the running app recovery-only, so on success
 * the session leaves the ordinary screens for the "ready for 0.5.0" screen.
 *
 * Also offered on the recovery screen when the archive is one this version
 * can prepare (`RollbackOffer`): preparing doesn't need the archive open.
 */
import { useMutation, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useSession } from '@/app/session'
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
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import { archiveAccessClient } from '@/lib/backend-client/archive-access'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { Refusal } from './affected-file'
import { Notice } from './notice'

const NEEDS_CURRENT = 'archive-current-password-required'
const WRONG_PASSWORD = 'archive-password-wrong'
const UNSUPPORTED = 'archive-rollback-unsupported'
const CROSS_DEVICE = 'archive-handoff-cross-device'

const previewKey = ['archive-rollback-preview'] as const

export function RollbackDialog({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <RollbackForm onClose={onClose} />}
    </Dialog>
  )
}

function RollbackForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const format = useFormat()
  const session = useSession()
  const preview = useQuery({
    queryKey: previewKey,
    queryFn: archiveAccessClient.previewRollback,
    staleTime: 0,
    gcTime: 0,
  })
  const [password, setPassword] = useState('')
  const [askCurrent, setAskCurrent] = useState(false)
  const prepare = useMutation({
    mutationFn: () =>
      archiveAccessClient.prepareRollback(askCurrent ? password : null),
    onSuccess: (packageFolder) => session.showPrepared(packageFolder),
    onError: (error) => {
      if (commandErrorCode(error) === NEEDS_CURRENT) setAskCurrent(true)
    },
  })
  const code = prepare.error ? commandErrorCode(prepare.error) : null
  const previewCode = preview.error ? commandErrorCode(preview.error) : null
  const busy = prepare.isPending || prepare.isSuccess
  const ready = preview.isSuccess && (!askCurrent || password.length > 0)

  return (
    <DialogContent
      className="sm:max-w-lg"
      showCloseButton={!busy}
      onInteractOutside={(event) => busy && event.preventDefault()}
      onEscapeKeyDown={(event) => busy && event.preventDefault()}
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (ready && !busy) prepare.mutate()
        }}
      >
        <DialogHeader>
          <DialogTitle>{t('archiveAccess.rollback.title')}</DialogTitle>
          <DialogDescription>
            {t('archiveAccess.rollback.intro')}
          </DialogDescription>
        </DialogHeader>

        {preview.isPending ? (
          <div
            className="flex flex-col gap-2"
            aria-busy
            aria-label={t('archiveAccess.rollback.checking')}
          >
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-4 w-2/3" />
          </div>
        ) : preview.isError ? (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {previewCode === UNSUPPORTED
              ? t('archiveAccess.rollback.unsupported', {
                  message: describeError(preview.error),
                })
              : previewCode === CROSS_DEVICE
                ? t('archiveAccess.rollback.crossDevice')
                : t('archiveAccess.rollback.loadFailed', {
                    message: describeError(
                      preview.error,
                      'preview_archive_rollback',
                    ),
                  })}
          </p>
        ) : (
          <div className="flex flex-col gap-3 text-[13px]">
            <p>{t('archiveAccess.rollback.sees')}</p>
            <div className="flex flex-col gap-1.5">
              <p>{t('archiveAccess.rollback.notSees')}</p>
              <p
                data-rollback-package
                className="rounded-lg bg-muted px-3 py-2 font-mono text-xs break-all"
              >
                {preview.data.packageFolder}
              </p>
              <p className="text-muted-foreground">
                {t('archiveAccess.rollback.conversations', {
                  size: format.bytes(preview.data.conversationBytes),
                })}
              </p>
            </div>
            {preview.data.copyFoldersAndFiles.length > 0 && (
              <div className="flex flex-col gap-1.5">
                <p>{t('archiveAccess.rollback.copies')}</p>
                <ul
                  aria-label={t('archiveAccess.rollback.copies')}
                  className="max-h-32 overflow-y-auto rounded-lg bg-muted p-2.5 font-mono text-xs"
                >
                  {preview.data.copyFoldersAndFiles.map((item) => (
                    <li key={item} className="break-all">
                      {item}
                    </li>
                  ))}
                </ul>
              </div>
            )}
            <ul className="flex list-disc flex-col gap-1 pl-5 text-muted-foreground">
              <li>{t('archiveAccess.rollback.wipe')}</li>
              {preview.data.encrypted && (
                <li>{t('archiveAccess.rollback.password')}</li>
              )}
              <li>{t('archiveAccess.rollback.back')}</li>
            </ul>
          </div>
        )}

        {askCurrent && (
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="rollback-current-password">
              {t('archiveAccess.currentPassword')}
            </Label>
            <Input
              id="rollback-current-password"
              type="password"
              autoComplete="current-password"
              value={password}
              disabled={busy}
              aria-invalid={code === WRONG_PASSWORD}
              aria-describedby="rollback-current-password-hint"
              onChange={(event) => setPassword(event.target.value)}
            />
            <p
              id="rollback-current-password-hint"
              className="text-xs text-muted-foreground"
            >
              {t('archiveAccess.currentPasswordHint')}
            </p>
          </div>
        )}

        {busy && (
          <div role="status" className="flex flex-col gap-1.5">
            <Progress
              className="h-1.5"
              aria-label={t('archiveAccess.rollback.running')}
            />
            <span className="text-xs text-muted-foreground">
              {t('archiveAccess.rollback.running')}
            </span>
          </div>
        )}

        {prepare.error && code !== NEEDS_CURRENT && (
          <Refusal error={prepare.error}>
            {code === WRONG_PASSWORD
              ? t('archiveAccess.wrongCurrentPassword')
              : code === CROSS_DEVICE
                ? t('archiveAccess.rollback.crossDevice')
                : t('archiveAccess.rollback.failed', {
                    message: describeError(
                      prepare.error,
                      'prepare_archive_rollback',
                    ),
                  })}
          </Refusal>
        )}

        {preview.isSuccess && (
          <Notice tone="warning">{t('archiveAccess.rollback.after')}</Notice>
        )}

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            disabled={busy}
            onClick={onClose}
          >
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={!ready || busy}>
            {t('archiveAccess.rollback.confirm')}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}

/**
 * The recovery screen's offer: shown only when this archive can be
 * prepared (the preview answers), so it never offers what would be refused.
 */
export function RollbackOffer() {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const preview = useQuery({
    queryKey: [...previewKey, 'offer'],
    queryFn: archiveAccessClient.previewRollback,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  })
  if (!preview.isSuccess || preview.data.needsChoice) return null
  return (
    <section
      aria-labelledby="rollback-offer"
      className="flex w-full items-center justify-between gap-4 rounded-xl border bg-card p-[18px] text-left shadow-card"
    >
      <div className="flex min-w-0 flex-col gap-[3px]">
        <h2 id="rollback-offer" className="text-sm font-semibold">
          {t('archiveAccess.rollback.rowTitle')}
        </h2>
        <p className="text-[13px] text-muted-foreground">
          {t('archiveAccess.rollback.rowDescription')}
        </p>
      </div>
      <Button size="sm" variant="outline" onClick={() => setOpen(true)}>
        {t('archiveAccess.rollback.action')}
      </Button>
      <RollbackDialog open={open} onClose={() => setOpen(false)} />
    </section>
  )
}
