/**
 * Design change: the 「自動備份」 card leaves this page for Settings › 排程與背景工作
 * (data-spaces.md §4). What stays is one row at the foot of the page saying
 * what the schedule is and when it runs next, with a link to change it. Same
 * markup as the Agent space's row (`pages/backup/space-backup.tsx`).
 */
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { time } from './format'

export function ScheduleRow({ on, next }: { on: boolean; next: number }) {
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border bg-card px-5 py-3 shadow-card">
      <span className="text-sm font-semibold">自動備份</span>
      <span className="text-[13px] text-muted-foreground tabular">{on ? `每小時 · 下次 ${time(next)}` : '自動備份關著'}</span>
      <Button variant="outline" size="sm" className="ml-auto" asChild>
        <Link to="/settings/background">{on ? '更改排程' : '設定排程'}</Link>
      </Button>
    </div>
  )
}
