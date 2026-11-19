/**
 * Make a safety copy of the archive now: check what it would hold
 * (`preview_archive_safety_copy`, reading only), then make it
 * (`create_archive_safety_copy`). It works while saved Agent conversations
 * can't be read, so browsing history keeps a way back: such a copy keeps
 * their files byte for byte, is marked in the lists and restores browsing;
 * whether what it holds can repair the conversations is found out when a
 * repair checks it. The dialog says that before the copy is made.
 *
 * The copy is listed in Restore from a safety copy and Free up space, and
 * counted in disk usage, so those lists are read again once it exists.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
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
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { agentRepairClient } from '@/lib/backend-client/agent-repair'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryKeys } from '@/lib/query'
import { waitForNextPaint } from '@/lib/wait-for-next-paint'
import { Refusal } from './affected-file'
import { Notice } from './notice'
import { reveal, revealLabelKey } from './reveal'

const previewKey = ['safety-copy-preview'] as const

export function SafetyCopyDialog({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <SafetyCopyForm onClose={onClose} />}
    </Dialog>
  )
}

function SafetyCopyForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const client = useQueryClient()
  const plan = useQuery({
    queryKey: previewKey,
    queryFn: agentRepairClient.previewSafetyCopy,
    staleTime: 0,
    gcTime: 0,
  })
  const create = useMutation({
    mutationFn: async (token: string) => {
      // The copying line paints before the backend starts copying.
      await waitForNextPaint()
      return agentRepairClient.createSafetyCopy(token)
    },
    onSuccess: (copy) => {
      toast.success(t('settingsStorage.safetyCopy.done'), {
        description: copy.path,
        action: {
          label: t(revealLabelKey()),
          onClick: () =>
            void reveal(copy.path, t('settingsStorage.location.failed')),
        },
      })
      void client.invalidateQueries({ queryKey: ['recovery-snapshots'] })
      void client.invalidateQueries({ queryKey: ['retention-preview'] })
      void client.invalidateQueries({ queryKey: queryKeys.dashboard })
      onClose()
    },
  })
  const stale =
    create.error !== null &&
    commandErrorCode(create.error) === 'conversation-repair-preview-stale'
  const unreadable = plan.data?.conversationsUnreadable === true

  return (
    <DialogContent
      className="sm:max-w-md"
      showCloseButton={!create.isPending}
      onInteractOutside={(event) => create.isPending && event.preventDefault()}
      onEscapeKeyDown={(event) => create.isPending && event.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>{t('settingsStorage.safetyCopy.title')}</DialogTitle>
        <DialogDescription>
          {t('settingsStorage.safetyCopy.intro')}
        </DialogDescription>
      </DialogHeader>

      {plan.isPending || plan.isFetching ? (
        <div className="flex flex-col gap-2">
          <span className="sr-only">
            {t('settingsStorage.safetyCopy.checking')}
          </span>
          <Skeleton aria-hidden className="h-4 w-40" />
          <Skeleton aria-hidden className="h-4 w-full" />
          <Skeleton aria-hidden className="h-10 w-full" />
        </div>
      ) : plan.isError ? (
        <Refusal error={plan.error}>
          {t('settingsStorage.safetyCopy.checkFailed', {
            message: describeError(plan.error, 'preview_archive_safety_copy'),
          })}
        </Refusal>
      ) : (
        <div data-safety-copy-plan className="flex flex-col gap-3 text-[13px]">
          <div className="flex flex-col gap-1">
            <p>{t('settingsStorage.safetyCopy.folder')}</p>
            <span className="font-mono text-xs break-all">
              {plan.data.directory}
            </span>
          </div>
          <p className="text-muted-foreground">
            {t('settingsStorage.safetyCopy.holds')}
          </p>
          {unreadable && (
            <Notice tone="warning">
              {t('settingsStorage.safetyCopy.unreadable')}
            </Notice>
          )}
          <p className="text-muted-foreground">
            {t('settingsStorage.safetyCopy.space')}
          </p>
        </div>
      )}

      {create.isPending && (
        <div role="status" className="flex flex-col gap-1.5">
          <Progress
            className="h-1.5"
            aria-label={t('maintenance.progressLabel')}
          />
          <p className="text-[13px] text-muted-foreground">
            {t('settingsStorage.safetyCopy.copying')}
          </p>
        </div>
      )}

      {create.error && (
        <Refusal error={create.error}>
          {stale
            ? t('settingsStorage.safetyCopy.stale')
            : commandErrorCode(create.error) ===
                'conversation-preservation-failed'
              ? t('settingsStorage.safetyCopy.preservationFailed', {
                  message: describeError(create.error),
                })
              : t('settingsStorage.safetyCopy.failed', {
                  message: describeError(
                    create.error,
                    'create_archive_safety_copy',
                  ),
                })}
        </Refusal>
      )}

      <DialogFooter>
        <Button variant="ghost" disabled={create.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        {stale || plan.isError ? (
          <Button
            disabled={plan.isFetching}
            onClick={() => {
              create.reset()
              void plan.refetch()
            }}
          >
            {t('settingsStorage.safetyCopy.checkAgain')}
          </Button>
        ) : (
          <Button
            disabled={!plan.data || plan.isFetching || create.isPending}
            onClick={() => plan.data && create.mutate(plan.data.previewToken)}
          >
            {create.isPending && <Spinner />}
            {t('settingsStorage.safetyCopy.confirm')}
          </Button>
        )}
      </DialogFooter>
    </DialogContent>
  )
}
