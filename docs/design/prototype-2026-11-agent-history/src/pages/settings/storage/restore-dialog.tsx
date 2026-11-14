/**
 * 從安全副本還原. Mirrors `src/features/settings/restore-dialog.tsx`: the
 * safety copies PathKeep took before risky changes, one to pick, its password
 * when it is encrypted, a warning, then the restore with progress and Stop.
 * After 釋放空間 deletes the snapshots there are no copies left, and the list
 * says so.
 *
 * Prototype: the password `wrong` fails the restore.
 */
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Skeleton } from '@/components/ui/skeleton'
import { dateTime, size } from '../pathkeep-kit/format'
import { MaintenanceProgress, operationSteps, StoppedLine } from '../pathkeep-kit/maintenance-progress'
import { ErrorLine, Notice } from '../pathkeep-kit/notice'
import type { RecoverySnapshot } from './data'

const opLabels: Record<string, string> = {
  rekey: '更改密碼之前',
  reconcile: '修復加密之前',
  import: '匯入之前',
  periodic: '定期副本',
}
const opLabel = (op: string) => opLabels[op] ?? '安全副本'

export function RestoreDialog({ open, snapshots, onClose }: { open: boolean; snapshots: RecoverySnapshot[]; onClose: () => void }) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <RestoreForm snapshots={snapshots} onClose={onClose} />}
    </Dialog>
  )
}

function RestoreForm({ snapshots, onClose }: { snapshots: RecoverySnapshot[]; onClose: () => void }) {
  const [loading, setLoading] = useState(true)
  const [chosenId, setChosenId] = useState<string | null>(null)
  const [password, setPassword] = useState('')
  const [running, setRunning] = useState(false)
  const [stopped, setStopped] = useState(false)
  const [failed, setFailed] = useState(false)
  const chosen = snapshots.find((item) => item.id === chosenId) ?? null
  const ready = chosen && (!chosen.encrypted || password.length > 0)

  useEffect(() => {
    const timer = setTimeout(() => setLoading(false), 500)
    return () => clearTimeout(timer)
  }, [])

  return (
    <DialogContent className="sm:max-w-md" showCloseButton={!running} onInteractOutside={(event) => running && event.preventDefault()} onEscapeKeyDown={(event) => running && event.preventDefault()}>
      <DialogHeader>
        <DialogTitle>從安全副本還原</DialogTitle>
        <DialogDescription>你的存檔會回到副本保存時的狀態。</DialogDescription>
      </DialogHeader>

      {loading ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : snapshots.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">還沒有任何安全副本。</p>
      ) : (
        <RadioGroup value={chosenId ?? ''} onValueChange={setChosenId} aria-label="從安全副本還原" className="max-h-64 gap-1 overflow-y-auto">
          {snapshots.map((item) => (
            <Label
              key={item.id}
              htmlFor={`restore-${item.id}`}
              className="flex items-center gap-3 rounded-lg border px-3 py-2.5 font-normal has-[[data-state=checked]]:border-foreground/30 has-[[data-state=checked]]:bg-muted"
            >
              <RadioGroupItem id={`restore-${item.id}`} value={item.id} />
              <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="font-medium">{opLabel(item.sourceOp)}</span>
                <span className="text-xs text-muted-foreground">
                  {item.createdAt ? dateTime(item.createdAt) : item.label}
                  {' · '}
                  {size(item.sizeBytes)}
                </span>
              </span>
            </Label>
          ))}
        </RadioGroup>
      )}

      {chosen?.encrypted && (
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="restore-password">這份副本的密碼</Label>
          <Input id="restore-password" type="password" autoComplete="current-password" value={password} disabled={running} onChange={(event) => setPassword(event.target.value)} />
        </div>
      )}

      {chosen && <Notice tone="warning">之後新增的內容不在副本裡。目前的存檔會被移到一旁，不會刪除。</Notice>}

      {running && (
        <MaintenanceProgress
          steps={operationSteps.restore}
          onDone={() => {
            setRunning(false)
            if (chosen?.encrypted && password === 'wrong') {
              setFailed(true)
              return
            }
            toast.success('存檔已還原。')
            toast('原型：PathKeep 在這裡重新開始，換上還原後的存檔。')
            onClose()
          }}
          onStopped={() => {
            setRunning(false)
            setStopped(true)
          }}
        />
      )}
      {stopped && <StoppedLine />}
      {failed && <ErrorLine>還原沒有完成。The password does not open this copy.</ErrorLine>}

      <DialogFooter>
        <Button variant="ghost" disabled={running} onClick={onClose}>
          取消
        </Button>
        <Button
          variant="destructive"
          disabled={!ready || running}
          onClick={() => {
            setStopped(false)
            setFailed(false)
            setRunning(true)
          }}
        >
          還原
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
