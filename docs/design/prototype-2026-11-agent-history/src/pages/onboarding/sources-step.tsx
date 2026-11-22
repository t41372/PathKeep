/**
 * Onboarding "來源": the sources of every chosen type, in two panes. The left
 * pane lists the chosen types, each with how many sources were found, how many
 * are selected, and anything that needs the user (a permission, an extra thing
 * PathKeep reads). The right pane lists the selected type's sources. Neither
 * pane makes the step itself scroll, so it holds for eight types and for
 * dozens of sources; with two types and nine sources it reads as a summary
 * with the first type open, and with one type the left pane is left out.
 *
 * Carried over from today's browsers step: the profile rows with the history
 * file's size, "需要權限" on a profile the system blocks, the Full Disk Access
 * notice with 打開設定 and 重新檢查 (and what each shows when it fails), the
 * notice alone when the system hides every profile, the empty and scan-failed
 * notices, and the rule that the step needs a source PathKeep can read. A
 * profile that becomes readable after 重新檢查 is not ticked for the user,
 * as today.
 */
import { FolderGit2, ScanSearch, ShieldAlert, TriangleAlert } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/cn'
import { harnessSources } from '../backup/sources-data'
import { InlineError } from './inline-error'
import type { StepKind, StepSource } from './sources-data'
import { StepHeading } from './steps'

type Selection = Record<string, Set<string>>

/** What looking for browsers came back with; anything but `found` is a prototype switch. */
export type Scan = 'found' | 'empty' | 'error' | 'access'

/** The list as the step shows it: the scan result and a granted permission applied. */
export function scanned(list: StepKind[], scan: Scan, granted: boolean): StepKind[] {
  return list.map((kind) => ({
    ...kind,
    sources: kind.id === 'browsing' && scan !== 'found' ? [] : granted ? kind.sources.map((s) => ({ ...s, blocked: false })) : kind.sources,
  }))
}

const picked = (kind: StepKind, chosen: Set<string>) => kind.sources.filter((s) => chosen.has(s.id))
const readable = (kind: StepKind, chosen: Set<string>) => picked(kind, chosen).filter((s) => !s.blocked)

function summary(kind: StepKind, chosen: Set<string>) {
  const found = kind.sources.length
  const count = picked(kind, chosen).length
  if (found === 0) return '沒有找到來源'
  if (count === 0) return `${found} ${kind.unit}，一個都沒選`
  if (count === found) return `${found} ${kind.unit}，都備份`
  return `${found} ${kind.unit}，選了 ${count} 個`
}

function KindRow({ kind, chosen, blocked, current, onSelect }: { kind: StepKind; chosen: Set<string>; blocked: number; current: boolean; onSelect: () => void }) {
  const Icon = kind.icon
  return (
    <button
      type="button"
      role="tab"
      id={`sources-tab-${kind.id}`}
      aria-selected={current}
      aria-controls="sources-panel"
      tabIndex={current ? 0 : -1}
      onClick={onSelect}
      className={cn(
        'flex w-full items-start gap-3 rounded-lg px-3 py-2.5 text-left transition-colors outline-none focus-visible:ring-[3px] focus-visible:ring-ring/40',
        current ? 'bg-muted' : 'hover:bg-muted/50',
      )}
    >
      <span className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md bg-background shadow-card">
        <Icon className="size-4" strokeWidth={1.75} />
      </span>
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate text-sm font-medium">{kind.label}</span>
        <span className={cn('text-xs', readable(kind, chosen).length === 0 ? 'text-brand' : 'text-muted-foreground')}>{summary(kind, chosen)}</span>
        {blocked > 0 && <span className="text-xs text-brand">{blocked} 個需要權限</span>}
        {kind.alsoReads && <span className="text-xs text-muted-foreground">{kind.alsoReads}</span>}
      </span>
    </button>
  )
}

function SourceRow({ source, checked, onChange }: { source: StepSource; checked: boolean; onChange: (on: boolean) => void }) {
  const id = `source-${source.id}`
  return (
    <li className="border-b last:border-b-0">
      <label htmlFor={id} className="flex cursor-pointer items-center gap-3.5 px-5 py-2.5 transition-colors hover:bg-muted/50">
        <Checkbox id={id} checked={checked} onCheckedChange={(value) => onChange(value === true)} className="size-[18px] rounded-[5px]" />
        {source.icon}
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-sm font-medium">{source.name}</span>
          <span className={cn('truncate text-xs text-muted-foreground', source.mono && 'font-mono text-[11.5px]')}>
            {source.detail}
            {source.blocked && <span className="font-sans"> · 需要權限</span>}
          </span>
        </span>
        {!source.blocked && (
          <span className="flex shrink-0 flex-col items-end text-right">
            <span className="font-mono text-xs text-muted-foreground tabular" title={source.amountTitle}>
              {source.amount}
            </span>
            {source.extra && <span className="text-xs text-muted-foreground">{source.extra}</span>}
          </span>
        )}
      </label>
    </li>
  )
}

function GroupHeading({ name, sources, chosen, onToggle }: { name: string; sources: StepSource[]; chosen: Set<string>; onToggle: (on: boolean) => void }) {
  const picked = sources.filter((s) => chosen.has(s.id)).length
  const id = `group-${name}`
  return (
    <li className="sticky top-0 z-10 border-b bg-card/95 backdrop-blur">
      <label htmlFor={id} className="flex cursor-pointer items-center gap-3.5 px-5 py-1.5">
        <Checkbox
          id={id}
          checked={picked === sources.length ? true : picked === 0 ? false : 'indeterminate'}
          onCheckedChange={(value) => onToggle(value === true)}
          className="size-[18px] rounded-[5px]"
        />
        <span className="text-xs font-semibold text-muted-foreground">{name}</span>
        <span className="text-xs text-muted-foreground">
          {picked} / {sources.length}
        </span>
      </label>
    </li>
  )
}

export function SourcesStep({
  list,
  selection,
  onSelection,
  scan,
  onScan,
  onGranted,
  onFound,
}: {
  /** Already passed through `scanned`. */
  list: StepKind[]
  selection: Selection
  onSelection: (kindId: string, next: Set<string>) => void
  scan: Scan
  onScan: (scan: Scan) => void
  onGranted: () => void
  /** Looking again found sources for a type that had none. */
  onFound: (kindId: string) => void
}) {
  const [activeId, setActiveId] = useState(list[0]?.id)
  const [filter, setFilter] = useState('')
  // Prototype-only: make one of the two buttons of the access notice fail.
  const [fails, setFails] = useState<'none' | 'open' | 'recheck'>('none')
  const [openFailed, setOpenFailed] = useState(false)
  const [recheckFailed, setRecheckFailed] = useState(false)
  const [checking, setChecking] = useState(false)
  const active = list.find((kind) => kind.id === activeId) ?? list[0]
  const shown = active
  if (!active) return null

  const chosen = selection[active.id] ?? new Set<string>()
  const long = shown.sources.length > 8
  const needle = filter.trim().toLowerCase()
  const visible = needle ? shown.sources.filter((s) => `${s.name} ${s.detail}`.toLowerCase().includes(needle)) : shown.sources
  const groups = [...new Set(visible.map((s) => s.group))]
  const browsing = active.id === 'browsing'
  // With one type there is nothing to switch between, so the list takes the whole card, as today.
  const several = list.length > 1
  // The system can hide every profile, or only some: either way the notice explains it.
  const needsAccess = (browsing && scan === 'access') || shown.sources.some((s) => s.blocked)
  const claude = harnessSources.find((h) => h.autoDelete)
  const unreadable = list.filter((kind) => readable(kind, selection[kind.id] ?? new Set()).length === 0)

  const set = (ids: string[], on: boolean) => {
    const next = new Set(chosen)
    ids.forEach((id) => (on ? next.add(id) : next.delete(id)))
    onSelection(active.id, next)
  }
  const move = (delta: number) => {
    const index = list.findIndex((kind) => kind.id === active.id)
    const next = list[(index + delta + list.length) % list.length]
    setActiveId(next.id)
    setFilter('')
    document.getElementById(`sources-tab-${next.id}`)?.focus()
  }
  const openSettings = () => {
    setOpenFailed(fails === 'open')
    if (fails !== 'open') toast('原型：這裡會打開系統設定的「完整磁碟取用權限」')
  }
  // Looks again, as the app does; a profile that became readable is left for the user to tick.
  const recheck = () => {
    setChecking(true)
    setRecheckFailed(false)
    setTimeout(() => {
      setChecking(false)
      if (fails === 'recheck') return setRecheckFailed(true)
      if (browsing) onScan('found')
      else onFound(active.id)
      if (needsAccess) onGranted()
    }, 700)
  }
  const recheckButton = (
    <Button size="sm" variant={needsAccess ? 'ghost' : 'outline'} disabled={checking} onClick={recheck}>
      <ScanSearch className={cn(checking && 'animate-pulse')} />
      重新檢查
    </Button>
  )

  return (
    <>
      <StepHeading title="找到這些來源" body="PathKeep 只讀這些資料夾，不改、不刪任何檔案。不想備份的可以取消勾選。" />
      <div className="flex h-[clamp(340px,calc(100vh-352px),600px)] overflow-hidden rounded-xl border bg-card shadow-card">
        {several && (
          <div
            role="tablist"
            aria-orientation="vertical"
            aria-label="資料類型"
            className="thin-scroll flex w-[252px] shrink-0 flex-col gap-0.5 overflow-y-auto border-r bg-muted/20 p-2"
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') (event.preventDefault(), move(1))
              if (event.key === 'ArrowUp') (event.preventDefault(), move(-1))
            }}
          >
            {list.map((kind) => (
              <KindRow
                key={kind.id}
                kind={kind}
                chosen={selection[kind.id] ?? new Set()}
                blocked={kind.sources.filter((s) => s.blocked).length}
                current={kind.id === active.id}
                onSelect={() => {
                  setActiveId(kind.id)
                  setFilter('')
                }}
              />
            ))}
          </div>
        )}
        <section id="sources-panel" role={several ? 'tabpanel' : 'group'} aria-labelledby={several ? `sources-tab-${active.id}` : 'sources-heading'} className="flex min-w-0 flex-1 flex-col">
          <header className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-5 py-3">
            <h2 id="sources-heading" className="text-sm font-semibold">
              {active.label}
            </h2>
            <span className="text-xs text-muted-foreground">{summary(shown, chosen)}</span>
            <span className="flex-1" />
            {long && (
              <Input
                value={filter}
                onChange={(event) => setFilter(event.target.value)}
                placeholder="篩選"
                aria-label={`篩選${active.label}的來源`}
                className="h-8 w-[150px] text-[13px]"
              />
            )}
            <Button size="sm" variant="ghost" disabled={shown.sources.length === 0 || picked(shown, chosen).length === shown.sources.length} onClick={() => set(shown.sources.map((s) => s.id), true)}>
              全選
            </Button>
            <Button size="sm" variant="ghost" disabled={picked(shown, chosen).length === 0} onClick={() => set(shown.sources.map((s) => s.id), false)}>
              全不選
            </Button>
          </header>
          {shown.sources.length === 0 ? (
            needsAccess ? (
              <span className="flex-1" />
            ) : (
              <div className="m-5 flex items-start gap-3 rounded-xl border px-4 py-3.5">
                {browsing && scan === 'error' ? <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" /> : <ScanSearch className="mt-0.5 size-4 shrink-0 text-muted-foreground" />}
                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="font-medium">{!browsing ? `找不到${active.label}的來源` : scan === 'error' ? 'PathKeep 無法掃描瀏覽器' : '找不到瀏覽器'}</span>
                  <span className="text-[13px] text-muted-foreground">
                    {!browsing
                      ? `PathKeep 會尋找 ${harnessSources.map((h) => h.name).join('、')} 的資料夾。裝好並使用過之後，再檢查一次；或回上一步先不選這個類型，之後找到了，首頁會問你。`
                      : scan === 'error'
                        ? '請再掃描一次。如果一直失敗，可以在「設定 → 關於」開啟記錄檔查看原因。'
                        : 'PathKeep 會尋找目前使用者安裝的 Chrome、Edge、Firefox、Safari 等瀏覽器。裝好並使用過之後，再掃描一次。'}
                  </span>
                </div>
                {recheckButton}
              </div>
            )
          ) : (
            <ul className="thin-scroll min-h-0 flex-1 overflow-y-auto">
              {visible.length === 0 && <li className="px-5 py-6 text-[13px] text-muted-foreground">沒有符合「{filter}」的來源。</li>}
              {long
                ? groups.map((group) => {
                    const members = visible.filter((s) => s.group === group)
                    return [
                      <GroupHeading key={`h-${group}`} name={group} sources={members} chosen={chosen} onToggle={(on) => set(members.map((s) => s.id), on)} />,
                      ...members.map((s) => <SourceRow key={s.id} source={s} checked={chosen.has(s.id)} onChange={(on) => set([s.id], on)} />),
                    ]
                  })
                : visible.map((s) => <SourceRow key={s.id} source={s} checked={chosen.has(s.id)} onChange={(on) => set([s.id], on)} />)}
            </ul>
          )}
          {needsAccess && (
            <div className="flex items-start gap-3 border-t bg-brand-soft px-5 py-3.5">
              <ShieldAlert className="mt-0.5 size-[18px] shrink-0 text-brand" />
              <div className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="text-sm font-medium">Safari 需要「完整磁碟取用權限」</span>
                <span className="text-[13px] leading-normal text-muted-foreground">打開系統設定 → 隱私權與安全性，把 PathKeep 加進去。不給也可以，會先跳過 Safari。</span>
                {openFailed && <InlineError title="無法打開系統設定" detail="原型：這裡是系統回報的原因。" />}
                {recheckFailed && <InlineError title="重新檢查失敗" detail="原型：這裡是掃描回報的原因。" />}
              </div>
              <div className="flex shrink-0 flex-col items-end gap-1">
                <Button size="sm" onClick={openSettings}>
                  打開設定
                </Button>
                {recheckButton}
              </div>
            </div>
          )}
          {active.id === 'agent' && (
            <div className="flex flex-col gap-2 border-t bg-muted/30 px-5 py-3.5 text-[13px] leading-relaxed text-muted-foreground">
              <span className="flex items-start gap-3">
                <FolderGit2 className="mt-0.5 size-4 shrink-0" />
                <span>
                  備份時也會讀每個專案資料夾的 <code className="font-mono text-[12px]">.git</code>，只看 repo 的根目錄、worktree 屬於哪個 repo、目前分支、遠端網址，用來把同一個專案的資料夾併在一起。不讀程式碼。之後可以在 Agent 對話的備份頁關掉。
                </span>
              </span>
              {claude && chosen.has(claude.id) && (
                <span className="flex items-start gap-3">
                  <TriangleAlert className="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-300" />
                  <span>
                    {claude.name} 預設 {claude.autoDelete!.days} 天後刪掉舊對話（可以在它自己的設定裡改）。第一次備份會把現在還在的 {(claude.chats - claude.gone).toLocaleString('zh-TW')} 段先存下來。
                  </span>
                </span>
              )}
            </div>
          )}
        </section>
      </div>
      {browsing && (
        <div className="flex flex-wrap gap-2">
          <label className="flex items-center gap-1.5 rounded-md border border-dashed px-2 py-1 text-[11px] text-muted-foreground">
            原型：掃描
            <select value={scan} onChange={(event) => onScan(event.target.value as Scan)} className="rounded bg-transparent text-[11px]">
              <option value="found">找到了</option>
              <option value="empty">一個都沒找到</option>
              <option value="error">掃描失敗</option>
              <option value="access">系統全擋住了</option>
            </select>
          </label>
          <label className="flex items-center gap-1.5 rounded-md border border-dashed px-2 py-1 text-[11px] text-muted-foreground">
            原型：按鈕
            <select value={fails} onChange={(event) => setFails(event.target.value as typeof fails)} className="rounded bg-transparent text-[11px]">
              <option value="none">都成功</option>
              <option value="open">打不開設定</option>
              <option value="recheck">重新檢查失敗</option>
            </select>
          </label>
        </div>
      )}
      {unreadable.length > 0 && (
        <p className="text-[13px] text-brand">
          「{unreadable.map((kind) => kind.label).join('」「')}」沒有選到 PathKeep 能讀取的來源。選一個，或回上一步取消這個類型。
        </p>
      )}
    </>
  )
}

/**
 * The step can be left only when every chosen type has a selected source
 * PathKeep can read, which is today's rule for browsers applied to each type.
 * `list` is the one the step shows (after `scanned`).
 */
export function sourcesComplete(list: StepKind[], selection: Selection) {
  return list.every((kind) => readable(kind, selection[kind.id] ?? new Set()).length > 0)
}
