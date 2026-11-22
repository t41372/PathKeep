/**
 * The two queue rows moved from Settings › Background work: 洞察與網頁細節
 * (counts, its jobs, 重建 behind a confirmation) and 搜尋索引與助理 (counts,
 * its jobs with 再執行一次 / pause / resume, or a way to Settings › AI when
 * it is off). Mirrors `InsightsQueueRow` / `AiQueueRow` in
 * `src/features/settings/background/queue-rows.tsx`.
 */
import { useState } from 'react'
import { Link } from 'react-router-dom'
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
import { useBackground } from './background-store'
import { JobList } from './job-list'
import { cancelJob, countSummary, countsOf, pauseJob, queueRebuild, retryJob, useJobs } from './jobs'

export function InsightsQueueRow() {
  const { paused, protoLoading, protoEmpty } = useBackground()
  const all = useJobs('insights')
  const jobs = protoEmpty ? [] : all
  const [confirm, setConfirm] = useState(false)
  return (
    <SettingRow
      title="洞察與網頁細節"
      description={
        protoLoading ? (
          <Skeleton className="mt-0.5 h-3.5 w-40" />
        ) : (
          <span className="flex flex-col">
            <span>從你的歷史紀錄中整理出瀏覽段落、搜尋和每日統計。</span>
            <span aria-live="polite">{countSummary(countsOf(jobs))}</span>
          </span>
        )
      }
      control={
        <Button size="sm" variant="outline" onClick={() => setConfirm(true)}>
          重建
        </Button>
      }
    >
      {!protoLoading && <JobList jobs={jobs} loading={false} onRetry={(j) => retryJob('insights', j)} onCancel={(j) => cancelJob('insights', j)} />}
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>重建洞察？</AlertDialogTitle>
            <AlertDialogDescription>
              這會讓洞察跟上你的全部歷史紀錄，已經是最新的部分會略過。若要從頭計算，請先清除下方的洞察資料。它在背景執行，你的歷史紀錄不會被改動。
              {paused && ' 背景工作已暫停，重建會等到你恢復後才開始。'}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction onClick={queueRebuild}>重建</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}

export function AiQueueRow() {
  const { aiOn, protoLoading, protoEmpty } = useBackground()
  const all = useJobs('ai')
  const jobs = protoEmpty ? [] : all
  const hasJobs = jobs.length > 0
  return (
    <SettingRow
      title="搜尋索引與助理"
      description={
        protoLoading ? (
          <Skeleton className="mt-0.5 h-3.5 w-40" />
        ) : !aiOn && !hasJobs ? (
          '依意思搜尋和助理都已關閉。'
        ) : (
          <span className="flex flex-col">
            <span>建立依意思搜尋用的索引，並執行排隊中的助理問題。</span>
            <span aria-live="polite">{countSummary(countsOf(jobs))}</span>
          </span>
        )
      }
      control={
        !aiOn && (
          <Button size="sm" variant="outline" asChild>
            <Link to="/settings/ai">AI 設定</Link>
          </Button>
        )
      }
    >
      {!protoLoading && (aiOn || hasJobs) && (
        <JobList
          jobs={jobs}
          loading={false}
          retryLabel={{ text: '再執行一次', aria: (name) => `再執行一次：${name}` }}
          onRetry={(j) => retryJob('ai', j)}
          onCancel={(j) => cancelJob('ai', j)}
          onPause={pauseJob}
        />
      )}
    </SettingRow>
  )
}
