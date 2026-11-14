/**
 * Sources: every browser profile PathKeep found, with a switch to include it
 * in backups; where a missing Safari permission or a failed scan is explained.
 * Mirrors `src/features/backup/sources-card.tsx` as it is today: profiles
 * grouped by browser (a browser with one profile is one row; several get a
 * summing row, closed when more than three and none needs permission),
 * profiles without a history file gathered last and closed.
 */
import { ChevronRight, ScanSearch, ShieldAlert, TriangleAlert } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { BrowserIcon } from '@/lib/browser-icons'
import { cn } from '@/lib/cn'
import { number, relative } from './format'
import { useScene } from './scene'
import { initiallySelected, profiles as allProfiles, stats, type Profile, type Stat } from './sources-data'

type SourceStatus = 'ok' | 'needsPermission' | 'paused' | 'notFound'
type StatsState = 'loading' | 'ready' | 'failed'

const statusLabel: Record<SourceStatus, string> = {
  ok: '正常',
  needsPermission: '需要授權',
  paused: '不備份',
  notFound: '找不到',
}

function sourceStatus(profile: Profile, selected: boolean): SourceStatus {
  if (!profile.historyExists) return 'notFound'
  if (profile.historyReadable === false) return 'needsPermission'
  return selected ? 'ok' : 'paused'
}

const statusRank: Record<SourceStatus, number> = { needsPermission: 3, paused: 2, notFound: 1, ok: 0 }

const statusStyle: Record<SourceStatus, string> = {
  ok: 'bg-green/12 text-green',
  needsPermission: 'bg-brand-soft text-brand',
  paused: 'bg-muted text-muted-foreground',
  notFound: 'bg-muted text-muted-foreground',
}

/** `openFullDiskAccess`: opens System Settings; only a failure would toast. */
function openFullDiskAccess() {
  toast('原型：這裡會打開「系統設定 → 隱私權與安全性 → 完整磁碟取用權限」')
}

/** `useRescan`: reads the snapshot again and says how many profiles it found. */
function useRescan(count: number) {
  const [pending, setPending] = useState(false)
  return {
    pending,
    run: () => {
      setPending(true)
      window.setTimeout(() => {
        setPending(false)
        toast.success(`掃描完成，找到 ${count} 個瀏覽器使用者。`)
      }, 700)
    },
  }
}

interface RowProps {
  statById: Map<string, Stat>
  statsState: StatsState
  selected: Set<string>
  onToggle: (profileId: string, next: boolean) => void
  rescan: ReturnType<typeof useRescan>
}

function SourceRow({ profile, rows, nested = false }: { profile: Profile; rows: RowProps; nested?: boolean }) {
  const selected = rows.selected.has(profile.profileId)
  const stat = rows.statById.get(profile.profileId)
  const status = sourceStatus(profile, selected)
  const canGrant = status === 'needsPermission' && profile.browserFamily === 'safari'
  const name = `${profile.browserName} ${profile.profileName}`

  return (
    <li className={cn('flex items-center gap-3.5 border-t py-3 pr-5 first:border-t-0', nested ? 'pl-[68px]' : 'pl-5')}>
      {!nested && <BrowserIcon browserName={profile.browserName} className="size-7" decorative />}
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-sm font-medium">{nested ? profile.profileName : profile.browserName}</span>
        {!nested && <span className="truncate text-xs text-muted-foreground">{profile.profileName}</span>}
      </div>
      <div className="hidden min-w-24 flex-col items-end text-right sm:flex">
        {rows.statsState === 'loading' ? (
          <Skeleton className="h-3.5 w-16" />
        ) : stat && rows.statsState === 'ready' ? (
          <>
            <span className="font-mono text-[13px] text-muted-foreground tabular">{number(stat.visitCount)}</span>
            <span className="text-xs text-muted-foreground">{stat.lastVisitAt ? `最近瀏覽 ${relative(stat.lastVisitAt)}` : '還沒有瀏覽記錄'}</span>
          </>
        ) : (
          <span className="text-muted-foreground" title={rows.statsState === 'failed' ? '暫時無法取得瀏覽次數' : undefined}>
            —
          </span>
        )}
      </div>
      {canGrant && (
        <Button size="sm" variant="outline" onClick={openFullDiskAccess}>
          授權
        </Button>
      )}
      {canGrant && (
        <Button size="sm" variant="ghost" disabled={rows.rescan.pending} onClick={rows.rescan.run}>
          重新檢查
        </Button>
      )}
      <Badge
        variant="secondary"
        className={cn('min-w-[76px] justify-center rounded-md border-0 font-normal', statusStyle[status])}
        title={status === 'needsPermission' ? profile.accessIssue : undefined}
      >
        {status === 'needsPermission' && <ShieldAlert />}
        {statusLabel[status]}
      </Badge>
      <Switch
        checked={selected && status !== 'notFound'}
        disabled={status === 'notFound'}
        onCheckedChange={(next) => rows.onToggle(profile.profileId, next)}
        aria-label={`備份 ${name}`}
      />
    </li>
  )
}

function ProfileGroup({
  label,
  browserName,
  profiles,
  missing = false,
  hint,
  rows,
}: {
  label: string
  browserName: string | null
  profiles: Profile[]
  missing?: boolean
  hint?: string
  rows: RowProps
}) {
  const statuses = profiles.map((p) => sourceStatus(p, rows.selected.has(p.profileId)))
  const worst = statuses.reduce((a, b) => (statusRank[b] > statusRank[a] ? b : a))
  const [open, setOpen] = useState(!missing && (profiles.length <= 3 || worst === 'needsPermission'))
  const on = profiles.filter((p) => p.historyExists && rows.selected.has(p.profileId)).length
  const visits = profiles.reduce((sum, p) => sum + (rows.statById.get(p.profileId)?.visitCount ?? 0), 0)
  const listId = `sources-${label.replace(/\W+/g, '-')}`
  return (
    <li className="border-t first:border-t-0">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={open ? `收起 ${label} 的使用者` : `顯示 ${label} 的使用者`}
        onClick={() => setOpen((value) => !value)}
        className="flex w-full items-center gap-3.5 px-5 py-3 text-left transition-colors hover:bg-muted/50"
      >
        <ChevronRight className={cn('size-4 shrink-0 text-muted-foreground transition-transform duration-150', open && 'rotate-90')} aria-hidden />
        {browserName && <BrowserIcon browserName={browserName} className="size-7" decorative />}
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-medium">{label}</span>
          <span className="truncate text-xs text-muted-foreground">{missing ? hint : `${profiles.length} 個使用者`}</span>
        </div>
        {!missing && (
          <>
            <span className="hidden min-w-24 text-right font-mono text-[13px] text-muted-foreground tabular sm:block">
              {rows.statsState === 'ready' ? number(visits) : '—'}
            </span>
            <span className="text-xs text-muted-foreground">{`已備份 ${on}/${profiles.length}`}</span>
            <Badge variant="secondary" className={cn('min-w-[76px] justify-center rounded-md border-0 font-normal', statusStyle[worst])}>
              {worst === 'needsPermission' && <ShieldAlert />}
              {statusLabel[worst]}
            </Badge>
          </>
        )}
      </button>
      {open && (
        <ul id={listId} className="border-t">
          {profiles.map((p) => (
            <SourceRow key={p.profileId} profile={p} rows={rows} nested={!missing} />
          ))}
        </ul>
      )}
    </li>
  )
}

function groupProfiles(list: Profile[]) {
  const byBrowser = new Map<string, Profile[]>()
  const missing: Profile[] = []
  for (const p of list) {
    if (!p.historyExists) {
      missing.push(p)
      continue
    }
    byBrowser.set(p.browserName, [...(byBrowser.get(p.browserName) ?? []), p])
  }
  return { browsers: [...byBrowser], missing }
}

const notices = {
  fullDiskAccess: {
    icon: ShieldAlert,
    title: 'Safari 需要完整磁碟取用權限',
    body: '在「系統設定 → 隱私權與安全性 → 完整磁碟取用權限」允許 PathKeep 之前，macOS 不會讓它讀取 Safari 的歷史記錄。',
  },
  error: {
    icon: TriangleAlert,
    title: 'PathKeep 無法掃描瀏覽器',
    body: '請再掃描一次。如果一直失敗，可以在「設定 → 關於」開啟記錄檔查看原因。',
  },
  empty: {
    icon: ScanSearch,
    title: '找不到瀏覽器',
    body: 'PathKeep 會尋找目前使用者安裝的 Chrome、Edge、Firefox、Safari 等瀏覽器。裝好並使用過之後，再掃描一次。',
  },
}

function DiscoveryNotice({ kind, rescan }: { kind: keyof typeof notices; rescan: ReturnType<typeof useRescan> }) {
  const { icon: Icon, title, body } = notices[kind]
  return (
    <div className="flex items-start gap-3 border-t px-5 py-4 first:border-t-0">
      <Icon className={cn('mt-0.5 size-4 shrink-0', kind === 'empty' ? 'text-muted-foreground' : 'text-brand')} />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-sm font-medium">{title}</span>
        <span className="text-[13px] text-muted-foreground">{body}</span>
      </div>
      {kind === 'fullDiskAccess' && (
        <Button size="sm" variant="outline" onClick={openFullDiskAccess}>
          授權
        </Button>
      )}
      <Button size="sm" variant="ghost" disabled={rescan.pending} onClick={rescan.run}>
        {kind === 'fullDiskAccess' ? '重新檢查' : '重新掃描'}
      </Button>
    </div>
  )
}

export function SourcesCard() {
  const scene = useScene()
  const [selectedIds, setSelectedIds] = useState(initiallySelected)
  const list = scene.sources === 'empty' ? [] : allProfiles
  const rescan = useRescan(list.length)
  const selected = new Set(selectedIds)
  const statsState: StatsState = scene.sources === 'statsLoading' ? 'loading' : scene.sources === 'statsFailed' ? 'failed' : 'ready'
  const statById = new Map(stats.map((s) => [s.profileId, s]))
  const { browsers, missing } = groupProfiles(list)
  const rows: RowProps = {
    statById,
    statsState,
    selected,
    rescan,
    onToggle: (profileId, next) =>
      setSelectedIds((ids) => (next ? [...new Set([...ids, profileId])] : ids.filter((id) => id !== profileId))),
  }

  return (
    <section className="overflow-hidden rounded-xl border bg-card shadow-card">
      <header className="flex items-center justify-between px-5 py-3.5">
        <h2 className="text-sm font-semibold">來源</h2>
        <Button size="sm" variant="outline" disabled={rescan.pending} onClick={rescan.run}>
          <ScanSearch className={cn(rescan.pending && 'animate-pulse')} />
          重新掃描
        </Button>
      </header>
      <ul className="border-t">
        {scene.sources === 'fullDiskAccess' && <DiscoveryNotice kind="fullDiskAccess" rescan={rescan} />}
        {scene.sources === 'discoveryError' && <DiscoveryNotice kind="error" rescan={rescan} />}
        {scene.sources === 'empty' && <DiscoveryNotice kind="empty" rescan={rescan} />}
        {browsers.map(([browserName, group]) =>
          group.length === 1 ? (
            <SourceRow key={group[0].profileId} profile={group[0]} rows={rows} />
          ) : (
            <ProfileGroup key={browserName} label={browserName} browserName={browserName} profiles={group} rows={rows} />
          ),
        )}
        {missing.length > 0 && (
          <ProfileGroup
            label={`${missing.length} 個使用者沒有瀏覽紀錄`}
            browserName={null}
            profiles={missing}
            missing
            hint="PathKeep 找到了這些使用者，但裡面沒有瀏覽紀錄檔：瀏覽器已移除，或這個使用者沒用過。"
            rows={rows}
          />
        )}
      </ul>
    </section>
  )
}
