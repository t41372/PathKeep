/**
 * Restore the whole archive from a safety copy PathKeep took before a risky
 * change (`list_recovery_snapshots`, then `run_full_archive_restore`). The
 * current archive is moved aside, not deleted.
 */
import { useMutation, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
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
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { archiveClient } from '@/lib/backend-client/archive'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import type { RecoverySnapshot } from '@/lib/types'
import { Notice } from './notice'

const knownOps = ['rekey', 'reconcile', 'import', 'periodic'] as const

function opKey(op: string) {
  return (knownOps as readonly string[]).includes(op)
    ? (op as (typeof knownOps)[number])
    : 'unknown'
}

export function RestoreDialog({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <RestoreForm onClose={onClose} />}
    </Dialog>
  )
}

function RestoreForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const format = useFormat()
  const session = useSession()
  const list = useQuery({
    queryKey: ['recovery-snapshots'],
    queryFn: archiveClient.listRecoverySnapshots,
    staleTime: 0,
    gcTime: 0,
  })
  const [chosenId, setChosenId] = useState<string | null>(null)
  const [password, setPassword] = useState('')
  const snapshots = (list.data ?? []).filter((item) => item.verifiedOpenable)
  const chosen = snapshots.find((item) => item.id === chosenId) ?? null

  const restore = useMutation({
    mutationFn: (snapshot: RecoverySnapshot) =>
      archiveClient.runFullArchiveRestore(
        { snapshotPath: snapshot.path },
        snapshot.encrypted ? password : null,
      ),
    onSuccess: async () => {
      toast.success(t('settingsStorage.restore.done'))
      onClose()
      // Every cached read described the archive that was just replaced.
      await session.restart()
    },
  })

  const ready = chosen && (!chosen.encrypted || password.length > 0)

  return (
    <DialogContent
      className="sm:max-w-md"
      showCloseButton={!restore.isPending}
      onInteractOutside={(event) => restore.isPending && event.preventDefault()}
      onEscapeKeyDown={(event) => restore.isPending && event.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>{t('settingsStorage.restore.title')}</DialogTitle>
        <DialogDescription>
          {t('settingsStorage.restore.intro')}
        </DialogDescription>
      </DialogHeader>

      {list.isPending ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : list.isError ? (
        <p role="alert" className="text-[13px] text-destructive">
          {t('settingsStorage.restore.loadFailed', {
            message: describeError(list.error, 'list_recovery_snapshots'),
          })}
        </p>
      ) : snapshots.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">
          {t('settingsStorage.restore.empty')}
        </p>
      ) : (
        <RadioGroup
          value={chosenId ?? ''}
          onValueChange={setChosenId}
          aria-label={t('settingsStorage.restore.title')}
          className="max-h-64 gap-1 overflow-y-auto"
        >
          {snapshots.map((item) => (
            <Label
              key={item.id}
              htmlFor={`restore-${item.id}`}
              className="flex items-center gap-3 rounded-lg border px-3 py-2.5 font-normal has-[[data-state=checked]]:border-foreground/30 has-[[data-state=checked]]:bg-muted"
            >
              <RadioGroupItem id={`restore-${item.id}`} value={item.id} />
              <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="font-medium">
                  {t(`settingsStorage.restore.op.${opKey(item.sourceOp)}`)}
                </span>
                <span className="text-xs text-muted-foreground">
                  {item.createdAt
                    ? format.date(item.createdAt, {
                        dateStyle: 'medium',
                        timeStyle: 'short',
                      })
                    : item.label}
                  {' · '}
                  {format.bytes(item.sizeBytes)}
                </span>
              </span>
            </Label>
          ))}
        </RadioGroup>
      )}

      {chosen?.encrypted && (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="restore-password">
            {t('settingsStorage.restore.password')}
          </Label>
          <Input
            id="restore-password"
            type="password"
            autoComplete="current-password"
            value={password}
            disabled={restore.isPending}
            onChange={(event) => setPassword(event.target.value)}
          />
        </div>
      )}

      {chosen && (
        <Notice tone="warning">{t('settingsStorage.restore.warning')}</Notice>
      )}

      {restore.isPending && (
        <div
          role="status"
          className="flex items-center gap-2 text-[13px] text-muted-foreground"
        >
          <Spinner />
          {t('settingsStorage.restore.running')}
        </div>
      )}

      {restore.error && (
        <p role="alert" className="text-[13px] text-destructive">
          {t('settingsStorage.restore.failed', {
            message: describeError(restore.error, 'run_full_archive_restore'),
          })}
        </p>
      )}

      <DialogFooter>
        <Button variant="ghost" disabled={restore.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          variant="destructive"
          disabled={!ready || restore.isPending}
          onClick={() => chosen && restore.mutate(chosen)}
        >
          {t('settingsStorage.restore.confirm')}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
