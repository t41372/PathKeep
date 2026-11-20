/**
 * The semantic index that stays on this computer while it isn't used (the
 * switch off, or semantic search off in Settings › AI): how much space it
 * takes and a way to free it. Freeing reads the size again
 * (`preview_free_agent_semantic_index`), shows it in a confirmation, and
 * deletes exactly what was shown (`free_agent_semantic_index`). When the
 * index changed in between, the confirmation reads the new size and asks
 * again instead of freeing on its own.
 *
 * Responsibilities: the size line, the confirmation and what each refusal
 * means. Not responsible for: whether the index is kept (the status in
 * `semantic-status.tsx`) or building it again (turning the switch on).
 */
import { useMutation, useQuery } from '@tanstack/react-query'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { useState } from 'react'
import { toast } from 'sonner'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { agentSemanticClient } from '@/lib/backend-client/agent-semantic'
import { describeError } from '@/lib/errors'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { refetchSemanticIndex } from '@/lib/queries/agent-semantic'

/** Why the backend wouldn't free it, in words; `null` for an unexpected error. */
const REFUSED: Record<string, MessageKey> = {
  // A build that was running when the switch went off is still stopping.
  'archive-busy': 'settingsAgent.ai.index.kept.busy',
  // The switch, or semantic search, is on again: the index is in use.
  'task-action-unavailable': 'settingsAgent.ai.index.kept.inUse',
}

function FreeDialog({
  opening,
  open,
  onClose,
}: {
  /** Which opening this is: each one reads the size afresh. */
  opening: number
  open: boolean
  onClose: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  // Read only while the dialog is open; each read replaces the one before.
  const preview = useQuery({
    queryKey: ['archive', 'agent-semantic', 'free-preview', opening],
    enabled: open,
    queryFn: agentSemanticClient.previewFree,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  })
  const [changed, setChanged] = useState(false)
  const free = useMutation({
    mutationFn: agentSemanticClient.free,
    onSuccess: (result) => {
      toast.success(
        t('settingsAgent.ai.index.kept.done', {
          size: format.bytes(Number(result.freedBytes)),
        }),
      )
      refetchSemanticIndex()
      onClose()
    },
    onError: (error) => {
      // The index changed after the size was read: read it again and ask
      // again, rather than free something the person didn't see.
      if (commandErrorCode(error) === 'preview-expired') {
        setChanged(true)
        void preview.refetch()
      }
    },
  })
  const code = commandErrorCode(preview.error ?? free.error)
  const refused =
    (preview.error ?? free.error) && code !== 'preview-expired'
      ? code && REFUSED[code]
        ? t(REFUSED[code])
        : t(
            preview.error
              ? 'settingsAgent.ai.index.kept.readFailed'
              : 'settingsAgent.ai.index.kept.failed',
            {
              message: describeError(preview.error ?? free.error),
            },
          )
      : null
  const kept = preview.data ? Number(preview.data.keptBytes) : null
  const size = kept === null ? '' : format.bytes(kept)
  const receipt =
    !preview.isFetching && kept !== null && kept > 0 ? preview.data : null

  return (
    <AlertDialogContent>
      <AlertDialogHeader>
        <AlertDialogTitle>
          {t('settingsAgent.ai.index.kept.title')}
        </AlertDialogTitle>
        {preview.isFetching ? (
          <div className="flex flex-col gap-1.5" role="status">
            <span className="sr-only">
              {t('settingsAgent.ai.index.kept.reading')}
            </span>
            <Skeleton aria-hidden className="h-4 w-full" />
            <Skeleton aria-hidden className="h-4 w-4/5" />
          </div>
        ) : (
          <AlertDialogDescription asChild>
            <div className="flex flex-col gap-2">
              {changed && kept !== null && (
                <p className="font-medium text-foreground">
                  {t('settingsAgent.ai.index.kept.changed')}
                </p>
              )}
              {kept === 0 ? (
                <p>{t('settingsAgent.ai.index.kept.nothing')}</p>
              ) : kept !== null ? (
                <p>{t('settingsAgent.ai.index.kept.body', { size })}</p>
              ) : null}
              {refused && (
                <p role="alert" className="text-destructive">
                  {refused}
                </p>
              )}
            </div>
          </AlertDialogDescription>
        )}
      </AlertDialogHeader>
      <AlertDialogFooter>
        <AlertDialogCancel disabled={free.isPending}>
          {t('common.cancel')}
        </AlertDialogCancel>
        {receipt && (
          <AlertDialogAction
            variant="destructive"
            aria-busy={free.isPending || undefined}
            disabled={free.isPending}
            onClick={(event) => {
              // Stays open until the backend answers.
              event.preventDefault()
              free.mutate(receipt.previewId)
            }}
          >
            {free.isPending
              ? t('settingsAgent.ai.index.kept.freeing')
              : t('settingsAgent.ai.index.kept.confirm', { size })}
          </AlertDialogAction>
        )}
      </AlertDialogFooter>
    </AlertDialogContent>
  )
}

/**
 * The kept index's size and "Free…", sliding in and out with it; nothing
 * when none is kept. `keptBytes` is `null` while there's nothing to say
 * (the index is in use, or its state isn't known).
 */
export function KeptSemanticIndex({ keptBytes }: { keptBytes: string | null }) {
  const { t } = useI18n()
  const format = useFormat()
  const still = !!useReducedMotion()
  const [open, setOpen] = useState(false)
  const [opening, setOpening] = useState(0)
  const bytes = Number(keptBytes ?? 0)
  return (
    <>
      <AnimatePresence initial={false}>
        {bytes > 0 && (
          <motion.div
            key="kept"
            initial={still ? { opacity: 0 } : { height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={still ? { opacity: 0 } : { height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: 'easeOut' }}
            className="overflow-hidden"
          >
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 pb-3">
              <span
                className="text-muted-foreground tabular"
                data-kept-bytes={keptBytes}
              >
                {t('settingsAgent.ai.index.kept.line', {
                  size: format.bytes(bytes),
                })}
              </span>
              <Button
                size="sm"
                variant="outline"
                aria-label={t('settingsAgent.ai.index.kept.freeLabel')}
                onClick={() => {
                  setOpening((count) => count + 1)
                  setOpen(true)
                }}
              >
                {t('settingsAgent.ai.index.kept.free')}
              </Button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      <AlertDialog open={open} onOpenChange={setOpen}>
        {opening > 0 && (
          <FreeDialog
            key={opening}
            opening={opening}
            open={open}
            onClose={() => setOpen(false)}
          />
        )}
      </AlertDialog>
    </>
  )
}
