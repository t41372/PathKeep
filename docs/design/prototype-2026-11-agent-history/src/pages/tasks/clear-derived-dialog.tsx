/**
 * 清除洞察資料？ Preview → confirm → clear: counts what goes (counted fresh
 * each time it opens), says what stays, then clears and offers 立即重建.
 * Mirrors `src/features/settings/background/clear-derived-dialog.tsx`. A
 * second clear finds nothing left, which shows the empty preview.
 */
import { TriangleAlert } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'
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
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { number } from '../schedule/format'
import { queueRebuild } from './jobs'

/** `src/features/settings/notice.tsx`. */
function Notice({ tone, children }: { tone: 'warning' | 'info'; children: ReactNode }) {
  return (
    <div className={tone === 'warning' ? 'flex items-start gap-2.5 rounded-lg bg-brand-soft p-3 text-[13px]' : 'flex items-start gap-2.5 rounded-lg bg-muted p-3 text-[13px] text-muted-foreground'}>
      {tone === 'warning' && <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />}
      <span>{children}</span>
    </div>
  )
}

let cleared = false
const rowLabels = {
  visits: (n: number) => `${number(n)} 次造訪的細節`,
  daily: (n: number) => `${number(n)} 筆每日統計`,
  structural: (n: number) => `${number(n)} 筆瀏覽段落、搜尋與回訪紀錄`,
  runtime: (n: number) => `${number(n)} 筆重建紀錄與儲存的檢視`,
}

export function ClearDerivedDialog({ onClose }: { onClose: () => void }) {
  const [counting, setCounting] = useState(true)
  const [clearing, setClearing] = useState(false)
  useEffect(() => {
    const timer = window.setTimeout(() => setCounting(false), 700)
    return () => window.clearTimeout(timer)
  }, [])
  const rows = (
    [
      ['visits', cleared ? 0 : 2_431_208],
      ['daily', cleared ? 0 : 3_912],
      ['structural', cleared ? 0 : 184_330],
      ['runtime', cleared ? 0 : 57],
    ] as const
  ).filter(([, n]) => n > 0)
  const total = rows.reduce((s, [, n]) => s + n, 0)

  function clear() {
    setClearing(true)
    window.setTimeout(() => {
      cleared = true
      onClose()
      toast.success('洞察資料已清除。重新建立之前洞察會是空的。', { action: { label: '立即重建', onClick: queueRebuild } })
    }, 900)
  }

  return (
    <AlertDialog open onOpenChange={(next) => !next && !clearing && onClose()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>清除洞察資料？</AlertDialogTitle>
          <AlertDialogDescription>PathKeep 會刪除為洞察計算出來的資料。在重新建立之前（你按「重建」，或下次打開 PathKeep 時），洞察會是空的。</AlertDialogDescription>
        </AlertDialogHeader>
        <div className="flex flex-col gap-3 text-[13px]">
          {counting ? (
            <div className="flex flex-col gap-1.5">
              <span className="text-muted-foreground">正在統計將刪除的內容…</span>
              <Skeleton className="h-16 w-full" />
            </div>
          ) : total === 0 ? (
            <p>目前沒有可清除的內容。</p>
          ) : (
            <div className="flex flex-col gap-1">
              <span className="font-medium">將刪除：</span>
              <ul aria-label="將刪除：" className="flex list-disc flex-col gap-0.5 pl-5">
                {rows.map(([key, n]) => (
                  <li key={key} className="tabular-nums">
                    {rowLabels[key](n)}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <Notice tone="info">會保留：你的歷史紀錄、加星、筆記、搜尋索引、網頁摘要和連結預覽。</Notice>
        </div>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={clearing}>取消</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            disabled={counting || total === 0 || clearing}
            onClick={(e) => {
              e.preventDefault()
              clear()
            }}
          >
            {clearing && <Spinner />}
            清除
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
