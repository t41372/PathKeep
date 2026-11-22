/**
 * Taking back what a wrong import added to Agent conversations
 * (`list_conversation_imports`, then `undo_conversation_import`). An import
 * keeps this computer's conversations and adds the file's (user decision
 * 2026-11-15); this is the way back that doesn't depend on the copy made
 * before the import, which Free up space may already have deleted: the list
 * lives in the archive.
 *
 * Each import that added conversations is a row: when, how many it added,
 * how many an undo would still move. Undo first says what goes to the trash
 * and what stays, then keeps a copy of them in a folder of their own and
 * moves them to the trash (nothing is deleted), and ends with that copy's
 * place.
 *
 * The row in Storage shows only once an import has added conversations.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import {
  migrationClient,
  type ConversationImport,
} from '@/lib/backend-client/migration'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryKeys } from '@/lib/query'
import { Refusal } from './affected-file'
import { reveal, revealLabelKey } from './reveal'

const importsKey = ['conversation-imports'] as const
const NOT_FOUND = 'conversation-import-not-found'

function useImports(enabled = true) {
  return useQuery({
    queryKey: importsKey,
    queryFn: migrationClient.listConversationImports,
    staleTime: 0,
    enabled,
  })
}

/** The Storage row; nothing until an import has added conversations. */
export function ConversationImportsRow() {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const imports = useImports()
  if (!imports.data || imports.data.length === 0) return null
  return (
    <SettingRow
      title={t('archiveAccess.imports.rowTitle')}
      description={t('archiveAccess.imports.rowDescription')}
      control={
        <Button size="sm" variant="outline" onClick={() => setOpen(true)}>
          {t('archiveAccess.imports.action')}
        </Button>
      }
    >
      <Dialog open={open} onOpenChange={setOpen}>
        {open && <ImportsList />}
      </Dialog>
    </SettingRow>
  )
}

function ImportsList() {
  const { t } = useI18n()
  const format = useFormat()
  const imports = useImports()
  const [undoing, setUndoing] = useState<ConversationImport | null>(null)

  if (undoing)
    return <UndoConfirm item={undoing} onBack={() => setUndoing(null)} />

  return (
    <DialogContent className="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{t('archiveAccess.imports.title')}</DialogTitle>
        <DialogDescription>
          {t('archiveAccess.imports.intro')}
        </DialogDescription>
      </DialogHeader>
      {imports.isPending ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-12 w-full" />
        </div>
      ) : imports.isError ? (
        <p
          role="alert"
          className="text-[13px] [overflow-wrap:anywhere] text-destructive"
        >
          {t('archiveAccess.imports.loadFailed', {
            message: describeError(imports.error, 'list_conversation_imports'),
          })}
        </p>
      ) : (
        <ul
          aria-label={t('archiveAccess.imports.title')}
          className="flex max-h-72 flex-col gap-1 overflow-y-auto"
        >
          {imports.data.map((item) => (
            <li
              key={item.id}
              data-import-id={item.id}
              className="flex items-center justify-between gap-3 rounded-lg border px-3 py-2.5"
            >
              <span className="flex min-w-0 flex-col gap-0.5">
                <span className="font-medium">
                  {t('archiveAccess.imports.imported', {
                    date: format.date(item.importedAt, {
                      dateStyle: 'medium',
                      timeStyle: 'short',
                    }),
                  })}
                </span>
                <span className="text-xs text-muted-foreground">
                  {[
                    t('archiveAccess.imports.added', { count: item.added }),
                    item.eligible === 0
                      ? t('archiveAccess.imports.takenBack')
                      : item.eligible < item.added
                        ? t('archiveAccess.imports.eligible', {
                            count: item.eligible,
                          })
                        : null,
                  ]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </span>
              {item.eligible > 0 && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => setUndoing(item)}
                >
                  {t('archiveAccess.imports.undo')}
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
      <DialogFooter>
        <DialogCloseButton />
      </DialogFooter>
    </DialogContent>
  )
}

function DialogCloseButton() {
  const { t } = useI18n()
  return (
    <DialogClose asChild>
      <Button variant="ghost">{t('common.close')}</Button>
    </DialogClose>
  )
}

function UndoConfirm({
  item,
  onBack,
}: {
  item: ConversationImport
  onBack: () => void
}) {
  const { t } = useI18n()
  const client = useQueryClient()
  const undo = useMutation({
    mutationFn: () => migrationClient.undoConversationImport(item.id),
    onSuccess: (result) => {
      const copyPath = result.copyPath
      if (result.moved === 0) toast.info(t('archiveAccess.imports.doneNothing'))
      else
        toast.success(
          t('archiveAccess.imports.done', { count: result.moved }),
          {
            description: copyPath
              ? t('archiveAccess.imports.copyAt', { path: copyPath })
              : undefined,
            duration: 10_000,
            action: copyPath
              ? {
                  label: t(revealLabelKey()),
                  onClick: () =>
                    void reveal(copyPath, t('settingsStorage.location.failed')),
                }
              : undefined,
          },
        )
      // The trash and the lists changed under every Agent screen.
      void client.invalidateQueries({ queryKey: importsKey })
      void client.invalidateQueries({ queryKey: queryKeys.archiveData })
      onBack()
    },
  })
  const busy = undo.isPending

  return (
    <DialogContent
      className="sm:max-w-md"
      showCloseButton={!busy}
      onInteractOutside={(event) => busy && event.preventDefault()}
      onEscapeKeyDown={(event) => busy && event.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>
          {t('archiveAccess.imports.confirmTitle', { count: item.eligible })}
        </DialogTitle>
        <DialogDescription>
          {t('archiveAccess.imports.confirmBody')}
        </DialogDescription>
      </DialogHeader>
      {busy && (
        <div role="status" className="flex flex-col gap-1.5">
          <Progress
            className="h-1.5"
            aria-label={t('archiveAccess.imports.running')}
          />
          <span className="text-xs text-muted-foreground">
            {t('archiveAccess.imports.running')}
          </span>
        </div>
      )}
      {undo.error && (
        <Refusal error={undo.error}>
          {commandErrorCode(undo.error) === NOT_FOUND
            ? t('archiveAccess.imports.notFound')
            : t('archiveAccess.imports.failed', {
                message: describeError(undo.error, 'undo_conversation_import'),
              })}
        </Refusal>
      )}
      <DialogFooter>
        <Button variant="ghost" disabled={busy} onClick={onBack}>
          {t('common.back')}
        </Button>
        <Button
          variant="destructive"
          disabled={busy}
          onClick={() => undo.mutate()}
        >
          {t('archiveAccess.imports.confirm')}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
