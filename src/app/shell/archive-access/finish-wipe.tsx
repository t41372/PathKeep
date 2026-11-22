/**
 * Finishing a "Delete all data" PathKeep 0.5 started and didn't finish
 * (`archive-wipe-confirmation-required`, `confirm_interrupted_wipe`).
 *
 * The person asked for that deletion in PathKeep 0.5, so finishing it is
 * their own choice, not one PathKeep makes for them. Its record can't prove
 * it belongs to this folder, though, so the backend won't finish it on its
 * own: this asks again, behind the same typed word as "Delete all data",
 * with the advice to copy the folder first. It is the only action on the
 * recovery screen that deletes anything.
 *
 * Afterwards nothing is set up, so PathKeep starts over at setup, as after
 * "Delete all data": the session starts again, or, where the desktop only
 * answers recovery commands, the whole app restarts.
 *
 * Not responsible for: what is left in the folder (the record names nothing
 * the screen can list; the folder's path is shown above it), or a wipe this
 * version started (the backend finishes or undoes those at startup).
 */
import { useMutation } from '@tanstack/react-query'
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
import { Spinner } from '@/components/ui/spinner'
import {
  dataWipeClient,
  WIPE_CONFIRMATION_WORD,
} from '@/lib/backend-client/data-wipe'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { AfterRepair, Panel, PathLine, Working } from './parts'

/** The backend's answer when the folder no longer holds that record. */
const NOT_PENDING = 'archive-wipe-confirmation-required'

export function FinishWipe() {
  const { t } = useI18n()
  const session = useSession()
  const [open, setOpen] = useState(false)
  const [typed, setTyped] = useState('')
  const finish = useMutation({
    mutationFn: () => dataWipeClient.finishInterrupted(typed),
    onSuccess: () => {
      // Only the recovery commands answer in a refused start: setup needs
      // the app started again.
      if (session.access?.recoveryOnly) setOpen(false)
      else void session.restart()
    },
  })
  const busy = finish.isPending
  const ready = typed === WIPE_CONFIRMATION_WORD

  if (finish.isSuccess && session.access?.recoveryOnly) {
    return (
      <Panel
        title={t('archiveAccess.finishWipe.title')}
        labelledBy="finish-wipe"
      >
        <AfterRepair text={t('archiveAccess.finishWipe.done')} />
      </Panel>
    )
  }

  return (
    <Panel title={t('archiveAccess.finishWipe.title')} labelledBy="finish-wipe">
      <p className="text-[13px] text-muted-foreground">
        {t('archiveAccess.finishWipe.body')}
      </p>
      <Button
        variant="destructive"
        className="self-start"
        onClick={() => {
          setTyped('')
          finish.reset()
          setOpen(true)
        }}
      >
        {t('archiveAccess.finishWipe.action')}
      </Button>
      <Dialog open={open} onOpenChange={(next) => !busy && setOpen(next)}>
        <DialogContent
          className="sm:max-w-md"
          showCloseButton={!busy}
          onInteractOutside={(event) => busy && event.preventDefault()}
          onEscapeKeyDown={(event) => busy && event.preventDefault()}
        >
          <form
            className="flex flex-col gap-4"
            onSubmit={(event) => {
              event.preventDefault()
              if (ready && !busy) finish.mutate()
            }}
          >
            <DialogHeader>
              <DialogTitle>
                {t('archiveAccess.finishWipe.confirmTitle')}
              </DialogTitle>
              <DialogDescription>
                {t('archiveAccess.finishWipe.confirmBody')}
              </DialogDescription>
            </DialogHeader>
            {session.access && (
              <PathLine
                label={t('archiveAccess.folder')}
                path={session.access.root}
              />
            )}
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="finish-wipe-confirm">
                {t('settingsStorage.wipe.typeWord', {
                  word: WIPE_CONFIRMATION_WORD,
                })}
              </Label>
              <Input
                id="finish-wipe-confirm"
                autoComplete="off"
                spellCheck={false}
                placeholder={WIPE_CONFIRMATION_WORD}
                value={typed}
                disabled={busy}
                onChange={(event) => setTyped(event.target.value)}
              />
            </div>
            {busy && <Working label={t('archiveAccess.finishWipe.running')} />}
            {finish.error && (
              <p
                role="alert"
                className="text-[13px] [overflow-wrap:anywhere] text-destructive"
              >
                {commandErrorCode(finish.error) === NOT_PENDING
                  ? t('archiveAccess.finishWipe.changed')
                  : t('archiveAccess.finishWipe.failed', {
                      message: describeError(
                        finish.error,
                        'confirm_interrupted_wipe',
                      ),
                    })}
              </p>
            )}
            <DialogFooter>
              <Button
                type="button"
                variant="ghost"
                disabled={busy}
                onClick={() => setOpen(false)}
              >
                {t('common.cancel')}
              </Button>
              <Button
                type="submit"
                variant="destructive"
                disabled={!ready || busy}
              >
                {busy && <Spinner />}
                {t('settingsStorage.wipe.confirm')}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </Panel>
  )
}
