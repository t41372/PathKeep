/**
 * Onboarding "來源": the sources of every chosen type, in two panes. The left
 * pane lists the chosen types, each with how many sources were found, how many
 * are selected, and anything that needs the user (a permission, an extra thing
 * PathKeep reads). The right pane lists the selected type's sources. Neither
 * pane makes the step itself scroll, so it holds for eight types and for
 * dozens of sources; with two types and nine sources it reads as a summary
 * with the first type open.
 *
 * Carried over from today's browsers step: the profile rows with the history
 * file's size, "需要權限" on a profile the system blocks, the Full Disk Access
 * notice with 打開設定 and 重新檢查, and the empty notice.
 */
import { FolderGit2, ScanSearch, ShieldAlert, TriangleAlert } from 'lucide-react'
import { useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/cn'
import { harnessSources } from '../backup/sources-data'
import type { StepKind, StepSource } from './sources-data'
import { StepHeading } from './steps'

type Selection = Record<string, Set<string>>

function summary(kind: StepKind, chosen: Set<string>) {
  const found = kind.sources.length
  if (found === 0) return '沒有找到來源'
  if (chosen.size === 0) return `${found} ${kind.unit}，一個都沒選`
  if (chosen.size === found) return `${found} ${kind.unit}，都備份`
  return `${found} ${kind.unit}，選了 ${chosen.size} 個`
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
        <span className={cn('text-xs', chosen.size === 0 && kind.sources.length > 0 ? 'text-brand' : 'text-muted-foreground')}>{summary(kind, chosen)}</span>
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

export function SourcesStep({ list, selection, onSelection }: { list: StepKind[]; selection: Selection; onSelection: (kindId: string, next: Set<string>) => void }) {
  const [activeId, setActiveId] = useState(list[0]?.id)
  const [filter, setFilter] = useState('')
  const [granted, setGranted] = useState(false)
  const active = list.find((kind) => kind.id === activeId) ?? list[0]
  // Granting access turns the blocked profile into an ordinary, selected one.
  const view = useMemo(() => (granted ? list.map((kind) => ({ ...kind, sources: kind.sources.map((s) => ({ ...s, blocked: false })) })) : list), [granted, list])
  const shown = view.find((kind) => kind.id === active?.id)
  if (!active || !shown) return null

  const chosen = selection[active.id] ?? new Set<string>()
  const long = shown.sources.length > 8
  const needle = filter.trim().toLowerCase()
  const visible = needle ? shown.sources.filter((s) => `${s.name} ${s.detail}`.toLowerCase().includes(needle)) : shown.sources
  const groups = [...new Set(visible.map((s) => s.group))]
  const blockedHere = shown.sources.filter((s) => s.blocked)
  const claude = harnessSources.find((h) => h.autoDelete)
  const emptyKinds = view.filter((kind) => kind.sources.length > 0 && (selection[kind.id]?.size ?? 0) === 0)

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
  const recheck = () => {
    setGranted(true)
    const safari = list.flatMap((kind) => kind.sources.filter((s) => s.blocked).map((s) => [kind.id, s.id] as const))
    safari.forEach(([kindId, id]) => onSelection(kindId, new Set([...(selection[kindId] ?? []), id])))
  }

  return (
    <>
      <StepHeading title="找到這些來源" body="PathKeep 只讀這些資料夾，不改、不刪任何檔案。不想備份的可以取消勾選。" />
      <div className="flex h-[clamp(340px,calc(100vh-352px),600px)] overflow-hidden rounded-xl border bg-card shadow-card">
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
          {view.map((kind) => (
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
        <section id="sources-panel" role="tabpanel" aria-labelledby={`sources-tab-${active.id}`} className="flex min-w-0 flex-1 flex-col">
          <header className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-5 py-3">
            <h2 className="text-sm font-semibold">{active.label}</h2>
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
            <Button size="sm" variant="ghost" disabled={chosen.size === shown.sources.length} onClick={() => set(shown.sources.map((s) => s.id), true)}>
              全選
            </Button>
            <Button size="sm" variant="ghost" disabled={chosen.size === 0} onClick={() => set(shown.sources.map((s) => s.id), false)}>
              全不選
            </Button>
          </header>
          {shown.sources.length === 0 ? (
            <div className="m-5 flex items-start gap-3 rounded-xl border px-4 py-3.5">
              <ScanSearch className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="font-medium">找不到瀏覽器</span>
                <span className="text-[13px] text-muted-foreground">PathKeep 會尋找目前使用者安裝的 Chrome、Edge、Firefox、Safari 等瀏覽器。裝好並使用過之後，再掃描一次。</span>
              </div>
              <Button size="sm" variant="outline">
                <ScanSearch />
                重新檢查
              </Button>
            </div>
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
          {blockedHere.length > 0 && (
            <div className="flex items-start gap-3 border-t bg-brand-soft px-5 py-3.5">
              <ShieldAlert className="mt-0.5 size-[18px] shrink-0 text-brand" />
              <div className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="text-sm font-medium">Safari 需要「完整磁碟取用權限」</span>
                <span className="text-[13px] leading-normal text-muted-foreground">打開系統設定 → 隱私權與安全性，把 PathKeep 加進去。不給也可以，會先跳過 Safari。</span>
              </div>
              <div className="flex shrink-0 flex-col items-end gap-1">
                <Button size="sm" onClick={() => toast('原型：這裡會打開系統設定的「完整磁碟取用權限」')}>
                  打開設定
                </Button>
                <Button size="sm" variant="ghost" onClick={recheck}>
                  <ScanSearch />
                  重新檢查
                </Button>
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
      {emptyKinds.length > 0 && (
        <p className="text-[13px] text-brand">
          「{emptyKinds.map((kind) => kind.label).join('」「')}」一個來源都沒選。至少選一個，或回上一步取消這個類型。
        </p>
      )}
    </>
  )
}

/** The step can be left only when every chosen type that has sources keeps at least one. */
export function sourcesComplete(list: StepKind[], selection: Selection) {
  return list.every((kind) => kind.sources.length === 0 || (selection[kind.id]?.size ?? 0) > 0)
}
