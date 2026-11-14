/**
 * The two kinds of background work that contact websites, as status:
 * 網頁摘要 (how many pages have one, its queue, 立即抓取) and 連結預覽
 * (coverage and 立即清理). Mirrors `PageSummariesRow` / `LinkPreviewsRow` in
 * `src/features/settings/background/online-work-rows.tsx`; link previews'
 * 何時抓取 and 保留圖片 are choices and stay in Settings › 排程與背景工作.
 *
 * When a feature is off the row links to its switch. The app's sentence says
 * 「在「一般」中開啟」; in this prototype the switches live in Settings ›
 * 瀏覽紀錄, so the button goes there.
 */
import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
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
import { Spinner } from '@/components/ui/spinner'
import { number } from '../schedule/format'
import { keepLabels, useBackground } from './background-store'

function OpenSwitch() {
  return (
    <Button size="sm" variant="outline" asChild>
      <Link to="/settings/browsing">一般設定</Link>
    </Button>
  )
}

export function PageSummariesRow() {
  const { summariesOn, protoLoading } = useBackground()
  const [queued, setQueued] = useState(12)
  const [fetching, setFetching] = useState(false)
  const [fetchedOnce, setFetchedOnce] = useState(false)
  const counts = ['1 項執行中', queued > 0 && `${queued} 項等待中`, '2 項失敗'].filter(Boolean)

  function fetchNow() {
    setFetching(true)
    window.setTimeout(() => {
      setFetching(false)
      if (fetchedOnce) toast.info('沒有需要抓取的網頁：它們已有摘要，或最近已經嘗試過。')
      else {
        setQueued((n) => n + 214)
        toast.success('已將 214 個網頁加入佇列。')
      }
      setFetchedOnce(true)
    }, 700)
  }

  return (
    <SettingRow
      title="網頁摘要"
      description={
        protoLoading ? (
          <Skeleton className="mt-0.5 h-3.5 w-40" />
        ) : !summariesOn ? (
          '已關閉。在「一般」中開啟「網頁摘要」，即可抓取你造訪過網頁的簡短摘要。'
        ) : (
          <span className="flex flex-col" aria-live="polite">
            <span>
              已為 {number(3120)} 個網頁產生摘要。{counts.length > 0 && ` ${counts.join(' · ')}`}
            </span>
            <span>把還沒有摘要、或摘要由舊版 PathKeep 產生的加星、最近造訪和最常造訪的網頁加入佇列。</span>
          </span>
        )
      }
      control={
        !protoLoading && !summariesOn ? (
          <OpenSwitch />
        ) : (
          <Button size="sm" variant="outline" disabled={protoLoading || fetching} onClick={fetchNow}>
            {fetching && <Spinner />}
            立即抓取
          </Button>
        )
      }
    />
  )
}

export function LinkPreviewsRow() {
  const { previewsOn, keep } = useBackground()
  const [counting, setCounting] = useState(true)
  const [confirm, setConfirm] = useState(false)
  const [cleaning, setCleaning] = useState(false)
  // Counting is a full pass over the archive's pages: read once per visit, never polled.
  useEffect(() => {
    if (!previewsOn) return
    setCounting(true)
    const timer = window.setTimeout(() => setCounting(false), 800)
    return () => window.clearTimeout(timer)
  }, [previewsOn])

  function clean() {
    setCleaning(true)
    window.setTimeout(() => {
      setCleaning(false)
      toast.success('已刪除 1,862 張圖片，釋放 214 MB。')
    }, 900)
  }

  return (
    <SettingRow
      title="連結預覽"
      description={
        !previewsOn
          ? '已關閉。在「一般」中開啟「連結預覽」，即可顯示網頁提供的分享圖片。'
          : counting
            ? '正在統計網頁…'
            : `${number(318_204)} 個網頁中有 ${number(146_930)} 個有預覽圖。目前已檢查 ${number(201_442)} 個。`
      }
      control={!previewsOn && <OpenSwitch />}
    >
      {previewsOn && (
        <div className="flex items-center justify-between gap-4 border-t pt-3 text-[13px]">
          <span className="text-muted-foreground">刪除「保留圖片」規則已不允許保留的圖片。</span>
          <Button size="sm" variant="outline" disabled={cleaning} onClick={() => setConfirm(true)}>
            {cleaning && <Spinner />}
            立即清理
          </Button>
        </div>
      )}
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>清理連結預覽？</AlertDialogTitle>
            <AlertDialogDescription>
              刪除「保留圖片」規則（{keepLabels[keep]}）已不允許保留的預覽圖，以及沒有網頁使用的圖片。網頁再次顯示時會重新下載。你的歷史紀錄不會被改動。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction onClick={clean}>立即清理</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}
