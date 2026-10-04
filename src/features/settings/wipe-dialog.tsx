/**
 * Delete all data: shows exactly what goes (`preview_wipe_all_data`), asks
 * for the word DELETE, deletes (`wipe_all_data`), then restarts the session,
 * which lands on onboarding because nothing is set up any more.
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
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { dataWipeClient } from '@/lib/backend-client/data-wipe'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { Notice } from './notice'

/** The backend checks for this exact word, in every language. */
const CONFIRMATION_WORD = 'DELETE'

export function WipeDialog({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <WipeForm onClose={onClose} />}
    </Dialog>
  )
}

function WipeForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const format = useFormat()
  const session = useSession()
  const [typed, setTyped] = useState('')
  const preview = useQuery({
    queryKey: ['wipe-preview'],
    queryFn: dataWipeClient.preview,
    staleTime: 0,
    gcTime: 0,
  })

  const wipe = useMutation({
    mutationFn: () => dataWipeClient.execute(typed),
    onSuccess: () => session.restart(),
  })

  const ready = preview.isSuccess && typed === CONFIRMATION_WORD

  return (
    <DialogContent
      className="sm:max-w-md"
      showCloseButton={!wipe.isPending}
      onInteractOutside={(event) => wipe.isPending && event.preventDefault()}
      onEscapeKeyDown={(event) => wipe.isPending && event.preventDefault()}
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (ready && !wipe.isPending) wipe.mutate()
        }}
      >
        <DialogHeader>
          <DialogTitle>{t('settingsStorage.wipe.title')}</DialogTitle>
          <DialogDescription>
            {t('settingsStorage.wipe.intro')}
          </DialogDescription>
        </DialogHeader>

        {preview.isPending ? (
          <div className="flex flex-col gap-2">
            <Skeleton className="h-5 w-3/4" />
            <Skeleton className="h-24 w-full" />
          </div>
        ) : preview.isError ? (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {t('settingsStorage.wipe.previewFailed', {
              message: describeError(preview.error, 'preview_wipe_all_data'),
            })}
          </p>
        ) : (
          <>
            <p className="text-[13px]">
              {t('settingsStorage.wipe.summary', {
                visits: t('common.visits', {
                  count: preview.data.visitCount,
                }),
                size: format.bytes(preview.data.totalBytes),
              })}
            </p>
            <ul className="max-h-40 overflow-y-auto rounded-lg bg-muted p-2.5 font-mono text-xs">
              {preview.data.items.map((item) => (
                <li key={item.path} className="flex justify-between gap-3">
                  <span className="min-w-0 break-all">{item.path}</span>
                  <span className="shrink-0 text-muted-foreground">
                    {format.bytes(item.bytes)}
                  </span>
                </li>
              ))}
            </ul>
            {preview.data.clearsKeychain && (
              <p className="text-[13px] text-muted-foreground">
                {t('settingsStorage.wipe.keychain')}
              </p>
            )}
            <Notice tone="warning">{t('settingsStorage.wipe.final')}</Notice>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="wipe-confirm">
                {t('settingsStorage.wipe.typeWord', {
                  word: CONFIRMATION_WORD,
                })}
              </Label>
              <Input
                id="wipe-confirm"
                autoComplete="off"
                spellCheck={false}
                placeholder={CONFIRMATION_WORD}
                value={typed}
                disabled={wipe.isPending}
                onChange={(event) => setTyped(event.target.value)}
              />
            </div>
          </>
        )}

        {wipe.error && (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {t('settingsStorage.wipe.failed', {
              message: describeError(wipe.error, 'wipe_all_data'),
            })}
          </p>
        )}

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            disabled={wipe.isPending}
            onClick={onClose}
          >
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            variant="destructive"
            disabled={!ready || wipe.isPending}
          >
            {wipe.isPending && <Spinner />}
            {t('settingsStorage.wipe.confirm')}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}
