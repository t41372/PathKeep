/**
 * 搜尋與洞察資料: what PathKeep worked out from the history, per part
 * (version, status, last build, why it is out of date), 清除… with a real
 * preview of what goes, and the extra page details plugins add. Clearing is
 * refused while a rebuild runs. Mirrors `InsightsDataRow` / `PageDetailsRow`
 * in `src/features/settings/background/derived-data-rows.tsx`.
 */
import { useState } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/cn'
import { NOW } from '~/data/fixture'
import { number, relative } from '../schedule/format'
import { useBackground } from './background-store'
import { ClearDerivedDialog } from './clear-derived-dialog'
import { useJobs } from './jobs'

type ModuleStatus = 'ready' | 'stale' | 'idle' | 'disabled' | 'running' | 'failed'
const statusLabels: Record<ModuleStatus, string> = { ready: '最新', stale: '已過期', idle: '尚未建立', disabled: '已關閉', running: '更新中', failed: '失敗' }
const statusStyle: Record<ModuleStatus, string> = {
  ready: 'bg-green/12 text-green',
  stale: 'bg-brand-soft text-brand',
  idle: 'bg-muted text-muted-foreground',
  disabled: 'bg-muted text-muted-foreground',
  running: 'bg-brand-soft text-brand',
  failed: 'bg-destructive/10 text-destructive',
}
const staleText = {
  'module-version-changed': '建立之後 PathKeep 已更新。',
  'missing-build-timestamp': '上次建立沒有順利完成。',
  'archive-data-changed': '建立之後有新的歷史紀錄。',
  'visibility-or-rollback-changed': '建立之後有造訪紀錄被隱藏或恢復。',
  'first-build-running': '第一次計算還沒完成，現在的數字只涵蓋已算好的瀏覽器。',
}

interface Module {
  id: string
  name: string
  version: number
  status: ModuleStatus
  builtAt: number | null
  stale?: keyof typeof staleText
}

const H = 3600_000
const modules: Module[] = [
  { id: 'visit-derived-facts', name: '網頁細節', version: 4, status: 'ready', builtAt: NOW - 78 * 60_000 },
  { id: 'daily-rollups', name: '每日統計', version: 3, status: 'stale', builtAt: NOW - 9 * H, stale: 'archive-data-changed' },
  { id: 'sessions', name: '瀏覽段落', version: 6, status: 'stale', builtAt: NOW - 4 * 24 * H, stale: 'module-version-changed' },
  { id: 'search-trails', name: '搜尋', version: 2, status: 'failed', builtAt: NOW - 2 * 24 * H, stale: 'missing-build-timestamp' },
  { id: 'refind-pages', name: '常回來的網頁', version: 2, status: 'running', builtAt: null, stale: 'first-build-running' },
  { id: 'activity-mix', name: '活動組成', version: 1, status: 'stale', builtAt: NOW - 30 * H, stale: 'visibility-or-rollback-changed' },
  { id: 'search-effectiveness', name: '有用的搜尋結果', version: 1, status: 'idle', builtAt: null },
  { id: 'domain-deep-dive', name: '網站細節', version: 1, status: 'disabled', builtAt: null },
]

export function InsightsDataRow() {
  const { protoLoading } = useBackground()
  const busy = useJobs('insights').some((j) => j.state === 'running')
  const [open, setOpen] = useState(false)
  return (
    <SettingRow
      title="洞察資料"
      description={busy ? '請先等正在執行的重建完成。' : '每個部分的版本，以及上次建立的時間。'}
      control={
        <Button size="sm" variant="outline" disabled={protoLoading || busy} onClick={() => setOpen(true)}>
          清除…
        </Button>
      }
    >
      {protoLoading ? (
        <Skeleton className="h-24 w-full" />
      ) : (
        <ul aria-label="洞察資料" className="flex flex-col border-t pt-1">
          {modules.map((m) => (
            <li key={m.id} className="flex items-center gap-3 border-b py-2 last:border-b-0" data-module={m.id} data-status={m.status}>
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="text-[13px] font-medium">{m.name}</span>
                <span className="text-xs text-muted-foreground">
                  版本 {m.version}
                  {m.builtAt && ` · ${relative(m.builtAt)}建立`}
                  {m.stale && ` · ${staleText[m.stale]}`}
                </span>
              </div>
              <Badge variant="secondary" className={cn('min-w-[76px] justify-center rounded-md border-0 font-normal', statusStyle[m.status])}>
                {statusLabels[m.status]}
              </Badge>
            </li>
          ))}
        </ul>
      )}
      {open && <ClearDerivedDialog onClose={() => setOpen(false)} />}
    </SettingRow>
  )
}

export function PageDetailsRow() {
  const { summariesOn, protoLoading } = useBackground()
  const plugins = [
    { id: 'title-normalization', name: '整理網頁標題', enabled: true, stored: 41_208, lastRun: NOW - 2 * H, lastError: null },
    {
      id: 'readable-content-refetch',
      name: '網頁摘要',
      enabled: summariesOn,
      stored: 3_120,
      lastRun: summariesOn ? NOW - 26 * H : null,
      lastError: summariesOn ? 'HTTP 429 Too Many Requests (developer.mozilla.org)' : null,
    },
  ]
  return (
    <SettingRow title="額外的網頁資訊" description="備份後為網頁補充的資訊，與你的歷史紀錄分開存放。">
      {protoLoading ? (
        <Skeleton className="h-12 w-full" />
      ) : (
        <ul className="flex flex-col border-t pt-1">
          {plugins.map((p) => (
            <li key={p.id} className="flex flex-col gap-0.5 border-b py-2 text-[13px] last:border-b-0">
              <div className="flex items-center justify-between gap-3">
                <span className="font-medium">{p.name}</span>
                <span className="text-xs text-muted-foreground tabular-nums">{p.enabled ? `${number(p.stored)} 個網頁` : '已關閉'}</span>
              </div>
              <span className="text-xs text-muted-foreground">{p.lastRun ? `${relative(p.lastRun)}執行` : '尚未執行'}</span>
              {p.lastError && <span className="text-xs break-words text-destructive">上次的問題：{p.lastError}</span>}
            </li>
          ))}
        </ul>
      )}
    </SettingRow>
  )
}
