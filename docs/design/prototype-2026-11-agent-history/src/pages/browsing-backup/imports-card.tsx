/**
 * 最近的匯入 (Takeout and browser files), each with undo or restore through a
 * review dialog: what the import added, from where, how many visits show
 * now, what it left out, each earlier undo and restore, and its audit
 * record. Mirrors `src/features/backup/imports-card.tsx`, including the
 * unfinished import, undo and restore the backend lists first.
 */
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { baseName, date, dayAndTime, number } from './format'
import { batchDetail, batches as initialBatches, type Batch } from './import-data'
import { PathLine } from './path-actions'
import { ImportEntries } from './run-warnings'
import { useScene } from './scene'

const kindLabel = (batch: Batch) => (batch.sourceKind === 'takeout' ? 'Google Takeout' : '瀏覽器歷史檔案')

type BatchAction = 'revert' | 'restore'

function batchAction(status: string): { action: BatchAction; finishing: boolean } | null {
  switch (status) {
    case 'running':
      return null
    case 'reverted':
      return { action: 'restore', finishing: false }
    case 'reverting':
      return { action: 'revert', finishing: true }
    case 'restoring':
      return { action: 'restore', finishing: true }
    default:
      // imported, interrupted, stopped, failed: undo hides whatever it added.
      return { action: 'revert', finishing: false }
  }
}

const endedPartway = (status: string) => status === 'interrupted' || status === 'stopped'

function actionLabel({ action, finishing }: { action: BatchAction; finishing: boolean }) {
  if (finishing) return action === 'revert' ? '完成復原' : '完成還原'
  return action === 'revert' ? '復原' : '還原'
}

function statusLine(batch: Batch) {
  if (batch.status === 'running') return '正在匯入…'
  if (batch.status === 'reverted') return '已復原，不再顯示在紀錄中。'
  if (batch.status === 'reverting') return '復原中途停止，部分記錄仍然顯示。'
  if (batch.status === 'restoring') return '還原中途停止，部分記錄仍然隱藏。'
  if (endedPartway(batch.status)) return `中途停止 · 已新增 ${number(batch.importedItems)} 筆`
  return `新增 ${number(batch.importedItems)} 筆 · 已有 ${number(batch.duplicateItems)} 筆`
}

export function ImportsCard({ running }: { running: Batch | null }) {
  const scene = useScene()
  const [list, setList] = useState(initialBatches)
  const [open, setOpen] = useState<Batch | null>(null)
  const shown = scene.importsEmpty ? (running ? [running] : []) : [...(running ? [running] : []), ...list]

  return (
    <SectionCard title="最近的匯入" subtitle="每次匯入都可以復原和還原，不會刪除任何內容。">
      {shown.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">還沒有匯入過。</p>
      ) : (
        <ul className="-my-1.5 flex flex-col">
          {shown.map((batch) => {
            const offer = batchAction(batch.status)
            return (
              <li key={batch.id} className="flex items-center gap-3 py-1.5 text-[13px]">
                <div className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate" title={batch.sourcePath}>
                    {kindLabel(batch)}
                    <span className="text-muted-foreground">
                      {' · '}
                      {baseName(batch.sourcePath)}
                    </span>
                  </span>
                  <span className="text-xs text-muted-foreground tabular">
                    {dayAndTime(batch.importedAt ?? batch.createdAt)}
                    {' · '}
                    {statusLine(batch)}
                  </span>
                </div>
                {offer && (
                  <Button size="sm" variant="ghost" onClick={() => setOpen(batch)}>
                    {actionLabel(offer)}
                  </Button>
                )}
              </li>
            )
          })}
        </ul>
      )}
      <Dialog open={open !== null} onOpenChange={(next) => !next && setOpen(null)}>
        {open && batchAction(open.status) && (
          <BatchReview
            batch={open}
            onClose={() => setOpen(null)}
            onDone={(next) => setList((all) => all.map((b) => (b.id === next.id ? next : b)))}
          />
        )}
      </Dialog>
    </SectionCard>
  )
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-lg bg-muted px-3 py-2">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-lg font-semibold tabular">{value}</span>
    </div>
  )
}

function BatchReview({ batch, onClose, onDone }: { batch: Batch; onClose: () => void; onDone: (batch: Batch) => void }) {
  const offer = batchAction(batch.status) ?? { action: 'revert' as const, finishing: false }
  const undone = offer.action === 'restore'
  // `preview_import_batch` loads first: skeletons, then the detail.
  const [loaded, setLoaded] = useState(false)
  const [pending, setPending] = useState(false)
  useEffect(() => {
    const timer = window.setTimeout(() => setLoaded(true), 450)
    return () => window.clearTimeout(timer)
  }, [])
  const detail = batchDetail(batch)

  function act() {
    setPending(true)
    window.setTimeout(() => {
      toast.success(undone ? '已還原匯入' : '已復原匯入')
      onDone({ ...batch, status: undone ? 'imported' : 'reverted', visibleItems: undone ? batch.importedItems : 0 })
      onClose()
    }, 1200)
  }

  return (
    <DialogContent
      className="sm:max-w-lg"
      showCloseButton={!pending}
      onInteractOutside={(event) => pending && event.preventDefault()}
      onEscapeKeyDown={(event) => pending && event.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>{`${dayAndTime(batch.importedAt ?? batch.createdAt)} 的匯入`}</DialogTitle>
        <DialogDescription>
          {offer.finishing
            ? undone
              ? '還原沒有完成就停止了。完成後，這次匯入新增的其餘記錄也會重新顯示。'
              : '復原沒有完成就停止了。完成後，這次匯入新增的其餘記錄也會隱藏。'
            : undone
              ? `還原會讓這次匯入新增的 ${number(batch.importedItems)} 筆瀏覽記錄重新顯示在紀錄中。`
              : `復原會隱藏這次匯入新增的 ${number(batch.visibleItems)} 筆瀏覽記錄。不會刪除任何內容，之後可以還原。`}
          {endedPartway(batch.status) && ' 這次匯入沒有完成就停止了。再次匯入同一個檔案會補上剩下的記錄，已新增的會略過。'}
        </DialogDescription>
      </DialogHeader>

      <div className="flex min-w-0 flex-col gap-4 text-[13px]">
        <div className="flex flex-col gap-1">
          <span className="text-xs text-muted-foreground">來源 · {kindLabel(batch)}</span>
          <PathLine path={batch.sourcePath} />
        </div>
        <div className="grid grid-cols-3 gap-2">
          <Fact label="新增" value={number(batch.importedItems)} />
          <Fact label="已有" value={number(batch.duplicateItems)} />
          <Fact label="顯示在紀錄中" value={number(batch.visibleItems)} />
        </div>
        {!loaded ? (
          <div className="flex flex-col gap-2">
            <Skeleton className="h-4 w-1/2" />
            <Skeleton className="h-16 w-full" />
          </div>
        ) : (
          <>
            {detail.previewRangeStart && detail.previewRangeEnd && (
              <span className="text-muted-foreground">{`從 ${date(detail.previewRangeStart)} 到 ${date(detail.previewRangeEnd)}`}</span>
            )}
            <div className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground">它新增的部分記錄</span>
              {detail.previewEntries.length === 0 ? (
                <span className="text-muted-foreground">它的記錄目前是隱藏的。</span>
              ) : (
                <ul className="flex flex-col">
                  {detail.previewEntries.slice(0, 5).map((entry) => (
                    <li key={`${entry.sourceVisitId}-${entry.visitedAt}`} className="flex items-baseline gap-2 py-0.5">
                      <span className="min-w-0 flex-1 truncate" title={entry.url}>
                        {entry.title || entry.url}
                      </span>
                      <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">{dayAndTime(entry.visitedAt)}</span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
            <ImportEntries recorded={detail.notes} />
            {batch.auditPath && (
              <div className="flex flex-col gap-0.5">
                <span className="text-xs text-muted-foreground">記錄</span>
                <PathLine path={batch.auditPath} />
              </div>
            )}
          </>
        )}
      </div>

      <DialogFooter>
        <Button variant="ghost" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button variant={undone ? 'default' : 'destructive'} disabled={pending || !loaded} onClick={act}>
          {pending && <Spinner />}
          {pending ? '正在處理…' : offer.finishing ? actionLabel(offer) : undone ? '還原匯入' : '復原匯入'}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
