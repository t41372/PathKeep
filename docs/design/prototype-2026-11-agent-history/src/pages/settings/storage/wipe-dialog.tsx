/**
 * 刪除所有資料. Mirrors `src/features/settings/wipe-dialog.tsx`: exactly
 * what goes (files with sizes, the keychain entry, the automatic backup the
 * system has installed), the word DELETE, then the delete; the app starts
 * over in onboarding. When the automatic backup could not be removed, the
 * dialog says so first and starts over when the user continues.
 *
 * Prototype: switches at the top choose whether the keychain holds the
 * password and whether removing the automatic backup fails.
 */
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { archiveBytes, browserSources } from '../../backup/sources-data'
import { count, size } from '../pathkeep-kit/format'
import { Notice } from '../pathkeep-kit/notice'
import { appPaths, scheduleItems, usePlatform } from '../pathkeep-kit/platform'
import { ProtoBar, ProtoSwitch } from '../pathkeep-kit/proto-bar'
import { INDEX_BYTES, type BucketId } from './data'

/** The backend checks for this exact word, in every language. */
const CONFIRMATION_WORD = 'DELETE'

export function WipeDialog({ open, buckets, onClose }: { open: boolean; buckets: Record<BucketId, number>; onClose: () => void }) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <WipeForm buckets={buckets} onClose={onClose} />}
    </Dialog>
  )
}

function WipeForm({ buckets, onClose }: { buckets: Record<BucketId, number>; onClose: () => void }) {
  const platform = usePlatform()
  const navigate = useNavigate()
  const { at } = appPaths(platform)
  const [loading, setLoading] = useState(true)
  const [typed, setTyped] = useState('')
  const [wiping, setWiping] = useState(false)
  const [keychain, setKeychain] = useState(true)
  const [scheduleFails, setScheduleFails] = useState(false)
  const [scheduleError, setScheduleError] = useState<string | null>(null)
  const schedule = scheduleItems(platform)

  useEffect(() => {
    const timer = setTimeout(() => setLoading(false), 600)
    return () => clearTimeout(timer)
  }, [])

  const browsing = archiveBytes.browsing ?? 0
  const items: [string, number][] = [
    [at('archive', 'history-vault.db'), Math.round(browsing * 0.82)],
    [at('archive', 'source-evidence.db'), Math.round(browsing * 0.18)],
    [at('derived'), (archiveBytes.agent ?? 0) + Math.round(INDEX_BYTES * 0.7)],
    [at('sidecars'), Math.round(INDEX_BYTES * 0.3)],
    [at('raw-snapshots'), buckets.snapshots],
    [at('staging'), buckets.staging],
    [at('quarantine'), buckets.quarantine],
    [at('audit'), 18_400_000],
    [at('exports'), buckets.exports],
    [at('models'), 91_200_000],
    [at('schedule'), 6_100],
    [at('app-lock-state.json'), 212],
    [at('app-lock-passcode.json'), 388],
    [at('config.json'), 4_870],
  ]
  const total = items.reduce((sum, [, bytes]) => sum + bytes, 0)
  const visits = browserSources.reduce((sum, source) => sum + source.visits, 0)
  const ready = !loading && typed === CONFIRMATION_WORD

  const startOver = () => {
    toast('原型：資料已刪除，PathKeep 從新手引導重新開始。')
    onClose()
    void navigate('/onboarding')
  }

  if (scheduleError) {
    // The data is gone; only the OS task is left. Say so, then start over.
    return (
      <DialogContent className="sm:max-w-md" showCloseButton={false} onInteractOutside={(event) => event.preventDefault()} onEscapeKeyDown={(event) => event.preventDefault()}>
        <DialogHeader>
          <DialogTitle>要刪除所有資料嗎？</DialogTitle>
          <DialogDescription className="[overflow-wrap:anywhere]">資料已刪除，但自動備份沒能移除。{scheduleError} 設定完成後，請到「備份」關閉它。</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button onClick={startOver}>繼續</Button>
        </DialogFooter>
      </DialogContent>
    )
  }

  return (
    <DialogContent className="sm:max-w-md" showCloseButton={!wiping} onInteractOutside={(event) => wiping && event.preventDefault()} onEscapeKeyDown={(event) => wiping && event.preventDefault()}>
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (!ready || wiping) return
          setWiping(true)
          setTimeout(() => {
            setWiping(false)
            if (scheduleFails && schedule.length > 0) setScheduleError(platform === 'macos' ? 'launchctl bootout failed: Operation not permitted (exit 1).' : 'schtasks /Delete failed: Access is denied. (exit 1)')
            else startOver()
          }, 900)
        }}
      >
        <DialogHeader>
          <DialogTitle>要刪除所有資料嗎？</DialogTitle>
          <DialogDescription>PathKeep 會刪除它儲存的一切，然後從頭開始。</DialogDescription>
        </DialogHeader>

        <ProtoBar>
          <ProtoSwitch label="鑰匙圈裡有存檔密碼" checked={keychain} onChange={setKeychain} />
          {schedule.length > 0 && <ProtoSwitch label="自動備份移除失敗" checked={scheduleFails} onChange={setScheduleFails} />}
        </ProtoBar>

        {loading ? (
          <div className="flex flex-col gap-2">
            <Skeleton className="h-5 w-3/4" />
            <Skeleton className="h-24 w-full" />
          </div>
        ) : (
          <>
            <p className="text-[13px]">
              將刪除 {count(visits)} 次瀏覽，共 {size(total)} 的檔案：
            </p>
            <ul className="max-h-40 overflow-y-auto rounded-lg bg-muted p-2.5 font-mono text-xs">
              {items.map(([path, bytes]) => (
                <li key={path} className="flex justify-between gap-3">
                  <span className="min-w-0 break-all">{path}</span>
                  <span className="shrink-0 text-muted-foreground">{size(bytes)}</span>
                </li>
              ))}
            </ul>
            {keychain && <p className="text-[13px] text-muted-foreground">存在鑰匙圈裡的存檔密碼也會被刪除。</p>}
            {schedule.length > 0 && (
              <div className="flex flex-col gap-1.5 text-[13px] text-muted-foreground">
                <p>自動備份也會關閉，並從系統中移除：</p>
                <ul className="rounded-lg bg-muted p-2.5 font-mono text-xs">
                  {schedule.map((item) => (
                    <li key={item} className="break-all">
                      {item}
                    </li>
                  ))}
                </ul>
              </div>
            )}
            <Notice tone="warning">此操作無法復原。如果之後可能還需要，請先匯出或搬移你的資料。</Notice>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="wipe-confirm">輸入 {CONFIRMATION_WORD} 以確認</Label>
              <Input id="wipe-confirm" autoComplete="off" spellCheck={false} placeholder={CONFIRMATION_WORD} value={typed} disabled={wiping} onChange={(event) => setTyped(event.target.value)} />
            </div>
          </>
        )}

        <DialogFooter>
          <Button type="button" variant="ghost" disabled={wiping} onClick={onClose}>
            取消
          </Button>
          <Button type="submit" variant="destructive" disabled={!ready || wiping}>
            {wiping && <Spinner />}
            永久刪除
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}
