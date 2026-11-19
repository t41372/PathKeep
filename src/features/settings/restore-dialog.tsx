/**
 * Restore browsing history from a safety copy PathKeep took before a risky
 * change (`list_recovery_snapshots`, then `run_full_archive_restore`). A
 * restore never touches saved Agent conversations (user decision
 * 2026-11-15): they stay as they are, and only the conversation repair uses
 * the conversations in a copy. The archive keeps its password and
 * encryption: an encrypted copy needs the password it was made with only to
 * be read. The browsing history it replaces becomes a safety copy too
 * ("before a restore"), so a restore can be undone from this same list. A
 * copy made while saved Agent conversations couldn't be read is marked; for
 * a restore that makes no difference. Copies that hold conversations only
 * (kept when a conversation file disappeared) are not offered here.
 */
import { useMutation, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useSession } from '@/app/session'
import { MaintenanceProgress } from '@/components/app/maintenance-progress'
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
import { archiveClient } from '@/lib/backend-client/archive'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { RecoverySnapshot } from '@/lib/types'
import { Refusal } from './affected-file'
import { copyLabelKey } from './copy-label'
import { Notice } from './notice'
import { announceReplaced } from './replaced-notice'

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
  const archiveEncrypted = useSnapshot().config.archiveMode === 'Encrypted'
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

  const [stopped, setStopped] = useState(false)
  const restore = useMutation({
    mutationFn: (snapshot: RecoverySnapshot) => {
      setStopped(false)
      return archiveClient.runFullArchiveRestore(
        { snapshotPath: snapshot.path },
        snapshot.encrypted ? password : null,
      )
    },
    onSuccess: async (report) => {
      announceReplaced(
        t,
        t('settingsStorage.restore.done'),
        report.previousCopyUnchecked,
      )
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
        <p
          role="alert"
          className="text-[13px] [overflow-wrap:anywhere] text-destructive"
        >
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
                  {t(
                    `settingsStorage.restore.op.${copyLabelKey(item.sourceOp)}`,
                  )}
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
                  {item.unchecked &&
                    ` · ${t('settingsStorage.restore.unchecked')}`}
                  {item.conversationsUnreadable &&
                    ` · ${t('settingsStorage.restore.conversationsUnreadable')}`}
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
            aria-describedby="restore-password-hint"
            value={password}
            disabled={restore.isPending}
            onChange={(event) => setPassword(event.target.value)}
          />
          <p
            id="restore-password-hint"
            className="text-xs text-muted-foreground"
          >
            {t('settingsStorage.restore.passwordHint')}
          </p>
        </div>
      )}

      {chosen?.conversationsUnreadable && (
        <p className="text-[13px] text-muted-foreground">
          {t('settingsStorage.restore.conversationsUnreadableNote')}
        </p>
      )}

      {chosen?.unchecked && (
        <p className="text-[13px] text-muted-foreground">
          {t('settingsStorage.restore.checkedFirst')}
        </p>
      )}

      {chosen && chosen.encrypted !== archiveEncrypted && (
        <p className="text-[13px] text-muted-foreground">
          {t(
            archiveEncrypted
              ? 'settingsStorage.restore.encryptsCopy'
              : 'settingsStorage.restore.staysUnencrypted',
          )}
        </p>
      )}

      {chosen && (
        <Notice tone="warning">{t('settingsStorage.restore.warning')}</Notice>
      )}

      {restore.isPending && (
        <MaintenanceProgress onStop={() => setStopped(true)} />
      )}

      {restore.error && stopped && (
        <p role="status" className="text-[13px] text-muted-foreground">
          {t('maintenance.stopped')}
        </p>
      )}
      {restore.error && !stopped && (
        <Refusal error={restore.error}>
          {t('settingsStorage.restore.failed', {
            message: describeError(restore.error, 'run_full_archive_restore'),
          })}
        </Refusal>
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
