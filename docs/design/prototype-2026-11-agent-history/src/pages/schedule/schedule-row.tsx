/**
 * Design change: the 「自動備份」 card lives in Settings › 排程與背景工作, so
 * every space's Backup page keeps one row that says what the schedule is
 * doing and leads there. It reads the same schedule status as the card, so
 * the two never disagree, and it shows a scheduler problem instead of hiding
 * it: fixing, granting and removing an old job stay on the card.
 */
import { TriangleAlert } from 'lucide-react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { time } from '~/data/format'
import { formatInterval } from './format'
import { useScheduleStatus } from './schedule-store'

const presets: Record<number, string> = { 1: '每小時', 6: '每 6 小時', 24: '每天' }

const problems: Record<string, string> = {
  mismatch: '已安裝的工作與 PathKeep 的設定不一致，備份可能不會準時執行。',
  'permission-warning': '系統還沒有允許這個排程工作執行，備份可能不會準時執行。',
  'legacy-install-detected': '舊版 PathKeep 留下了一個舊的自動備份工作。',
}

export function ScheduleRow() {
  const status = useScheduleStatus()
  const interval = presets[status.dueAfterHours] ?? `每 ${formatInterval(status.dueAfterHours)}`
  const manual = !status.applySupported || status.installState === 'manual-review'
  const problem = problems[status.installState]
  const next = status.installState === 'installed' && status.nextScheduledBackupAt ? Date.parse(status.nextScheduledBackupAt) : null
  const [text, action] = manual
    ? ['需手動設定', '看步驟']
    : problem
      ? [problem, '處理']
      : status.installState === 'installed'
        ? [next ? `${interval} · 下次 ${time(next)}` : interval, '更改排程']
        : ['自動備份關著', '設定排程']
  const warn = manual || Boolean(problem)
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border bg-card px-5 py-3 shadow-card">
      <span className="text-sm font-semibold">自動備份</span>
      <span className="flex min-w-0 items-center gap-1.5 text-[13px] text-muted-foreground tabular">
        {warn && <TriangleAlert className="size-3.5 shrink-0 text-brand" />}
        {text}
      </span>
      <Button variant="outline" size="sm" className="ml-auto" asChild>
        <Link to="/settings/background">{action}</Link>
      </Button>
    </div>
  )
}
