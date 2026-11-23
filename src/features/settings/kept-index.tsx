/**
 * A semantic index that stays on this computer while nothing uses it: how
 * much space it takes and a way to free it. Freeing reads the size again
 * (the index's preview command), shows it in a confirmation, and deletes
 * exactly what was shown. When the index changed in between, the
 * confirmation reads the new size and asks again instead of freeing on its
 * own. Browsing history's index (Settings › AI) and the Agent conversations'
 * (Settings › Agent conversations) each pass their own commands and words.
 *
 * Responsibilities: the size line, the confirmation and what each refusal
 * means. Not responsible for: whether an index is kept (each caller's
 * status) or building it again (turning semantic search on).
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
import type { FreeSemanticPreview } from '@/lib/backend-client/agent-semantic'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'

/** One kind of kept index: where its size is read and freed, and its words. */
export interface KeptIndexSource {
  /** Tells this index's reads apart in the cache. */
  name: string
  previewFree: () => Promise<FreeSemanticPreview>
  free: (
    previewId: string,
  ) => Promise<{ freedBytes: string; keptBytes: string }>
  /** Read again whatever shows the size, so the line goes once it's freed. */
  onFreed: () => void
  /** Why the backend wouldn't free it, by refusal code. */
  refused: Record<string, MessageKey>
  words: {
    /** `{size}` */
    line: MessageKey
    free: MessageKey
    freeLabel: MessageKey
    title: MessageKey
    reading: MessageKey
    /** `{size}` */
    body: MessageKey
    nothing: MessageKey
    changed: MessageKey
    /** `{size}` */
    confirm: MessageKey
    freeing: MessageKey
    /** `{size}` */
    done: MessageKey
    /** `{message}` */
    readFailed: MessageKey
    /** `{message}` */
    failed: MessageKey
  }
}

function FreeDialog({
  source,
  opening,
  open,
  onClose,
}: {
  source: KeptIndexSource
  /** Which opening this is: each one reads the size afresh. */
  opening: number
  open: boolean
  onClose: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const { words } = source
  // Read only while the dialog is open; each read replaces the one before.
  const preview = useQuery({
    queryKey: ['archive', source.name, 'free-preview', opening],
    enabled: open,
    queryFn: source.previewFree,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  })
  const [changed, setChanged] = useState(false)
  const free = useMutation({
    mutationFn: source.free,
    onSuccess: (result) => {
      toast.success(
        t(words.done, { size: format.bytes(Number(result.freedBytes)) }),
      )
      source.onFreed()
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
      ? code && source.refused[code]
        ? t(source.refused[code])
        : t(preview.error ? words.readFailed : words.failed, {
            message: describeError(preview.error ?? free.error),
          })
      : null
  const kept = preview.data ? Number(preview.data.keptBytes) : null
  const size = kept === null ? '' : format.bytes(kept)
  const receipt =
    !preview.isFetching && kept !== null && kept > 0 ? preview.data : null

  return (
    <AlertDialogContent>
      <AlertDialogHeader>
        <AlertDialogTitle>{t(words.title)}</AlertDialogTitle>
        {preview.isFetching ? (
          <div className="flex flex-col gap-1.5" role="status">
            <span className="sr-only">{t(words.reading)}</span>
            <Skeleton aria-hidden className="h-4 w-full" />
            <Skeleton aria-hidden className="h-4 w-4/5" />
          </div>
        ) : (
          <AlertDialogDescription asChild>
            <div className="flex flex-col gap-2">
              {changed && kept !== null && (
                <p className="font-medium text-foreground">
                  {t(words.changed)}
                </p>
              )}
              {kept === 0 ? (
                <p>{t(words.nothing)}</p>
              ) : kept !== null ? (
                <p>{t(words.body, { size })}</p>
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
            {free.isPending ? t(words.freeing) : t(words.confirm, { size })}
          </AlertDialogAction>
        )}
      </AlertDialogFooter>
    </AlertDialogContent>
  )
}

/**
 * The kept index's size and "Free…", sliding in and out with it; nothing
 * when none is kept. `keptBytes` is `null` while there's nothing to say
 * (the index is in use, or its state isn't known). The spacing that sets it
 * apart from what is around it goes in `className` (on the part that slides)
 * and `innerClassName` (on the part that is measured), so the slide moves
 * the spacing too.
 */
export function KeptIndex({
  source,
  keptBytes,
  className,
  innerClassName,
}: {
  source: KeptIndexSource
  keptBytes: string | null
  className?: string
  innerClassName?: string
}) {
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
            className={cn('overflow-hidden', className)}
          >
            <div
              className={cn(
                'flex flex-wrap items-center gap-x-3 gap-y-1.5',
                innerClassName,
              )}
            >
              <span
                className="text-muted-foreground tabular"
                data-kept-bytes={keptBytes}
              >
                {t(source.words.line, { size: format.bytes(bytes) })}
              </span>
              <Button
                size="sm"
                variant="outline"
                aria-label={t(source.words.freeLabel)}
                onClick={() => {
                  setOpening((count) => count + 1)
                  setOpen(true)
                }}
              >
                {t(source.words.free)}
              </Button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      <AlertDialog open={open} onOpenChange={setOpen}>
        {opening > 0 && (
          <FreeDialog
            key={opening}
            source={source}
            opening={opening}
            open={open}
            onClose={() => setOpen(false)}
          />
        )}
      </AlertDialog>
    </>
  )
}
