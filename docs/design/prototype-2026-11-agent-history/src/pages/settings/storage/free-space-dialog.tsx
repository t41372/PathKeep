/**
 * 釋放空間: what can be deleted, ticked by the user, then deleted. Mirrors
 * `src/features/settings/free-space-dialog.tsx`. Snapshots are opt-in (they
 * are the restore points), leftovers opt-out. What is deleted stays gone for
 * the rest of the visit, so opening it again shows 目前沒有需要清理的內容。
 * once everything is cleared.
 */
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { size } from '../pathkeep-kit/format'
import { Notice } from '../pathkeep-kit/notice'
import { bucketCopy, type BucketId } from './data'

export function FreeSpaceDialog({ open, buckets, onClose, onPruned }: { open: boolean; buckets: Record<BucketId, number>; onClose: () => void; onPruned: (ids: BucketId[]) => void }) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <FreeSpaceForm buckets={buckets} onClose={onClose} onPruned={onPruned} />}
    </Dialog>
  )
}

function FreeSpaceForm({ buckets: all, onClose, onPruned }: { buckets: Record<BucketId, number>; onClose: () => void; onPruned: (ids: BucketId[]) => void }) {
  const [loading, setLoading] = useState(true)
  const [pruning, setPruning] = useState(false)
  const [picked, setPicked] = useState<Set<BucketId> | null>(null)
  useEffect(() => {
    const timer = setTimeout(() => setLoading(false), 600)
    return () => clearTimeout(timer)
  }, [])

  const buckets = (Object.keys(all) as BucketId[]).filter((id) => all[id] > 0)
  const selected = picked ?? new Set(buckets.filter((id) => id !== 'snapshots'))
  const selectedBytes = buckets.filter((id) => selected.has(id)).reduce((sum, id) => sum + all[id], 0)

  const toggle = (id: BucketId, on: boolean) => {
    const next = new Set(selected)
    if (on) next.add(id)
    else next.delete(id)
    setPicked(next)
  }

  const prune = () => {
    setPruning(true)
    setTimeout(() => {
      toast.success(`已釋放 ${size(selectedBytes)}。`)
      onPruned([...selected])
      onClose()
    }, 700)
  }

  return (
    <DialogContent className="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>釋放空間</DialogTitle>
        <DialogDescription>選擇要刪除的內容。你的歷史不在這個清單裡。</DialogDescription>
      </DialogHeader>

      {loading ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-11 w-full" />
          <Skeleton className="h-11 w-full" />
        </div>
      ) : buckets.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">目前沒有需要清理的內容。</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {buckets.map((id) => (
            <li key={id} className="flex items-start gap-3 rounded-lg px-1 py-2">
              <Checkbox id={`prune-${id}`} className="mt-0.5" checked={selected.has(id)} disabled={pruning} onCheckedChange={(value) => toggle(id, value === true)} />
              <Label htmlFor={`prune-${id}`} className="flex min-w-0 flex-1 flex-col items-start gap-0.5 font-normal">
                <span className="flex w-full justify-between gap-3 font-medium">
                  {bucketCopy[id].title}
                  <span className="font-mono text-[13px] font-normal">{size(all[id])}</span>
                </span>
                <span className="text-[13px] text-muted-foreground">{bucketCopy[id].description}</span>
              </Label>
            </li>
          ))}
        </ul>
      )}

      {!loading && selected.has('snapshots') && <Notice tone="warning">刪除快照也會刪除可以用來還原的安全副本。</Notice>}

      <DialogFooter>
        <Button variant="ghost" disabled={pruning} onClick={onClose}>
          取消
        </Button>
        <Button variant="destructive" disabled={loading || pruning || selected.size === 0 || !buckets.length} onClick={prune}>
          {pruning && <Spinner />}
          刪除 {size(selectedBytes)}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
