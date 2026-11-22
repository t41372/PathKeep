/**
 * Above the conversation list: the full-text search box and one 篩選 button.
 * Everything that narrows or orders the list lives in that one menu; a
 * filter that is on shows as a small removable chip, nothing otherwise.
 */
import { Check, ChevronsUpDown, Search, SlidersHorizontal, X } from 'lucide-react'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command'
import { Kbd } from '@/components/ui/kbd'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/cn'
import { HarnessIcon, ProjectIcon } from '~/components/bits'
import { archive, harnesses, harnessOrder, projectsByRecent } from '~/data/archive'
import { ago, num } from '~/data/format'
import type { HarnessId } from '~/data/types'
import { flagLabels, groupLabels, rangeLabels, sortLabels, type Filters, type Flag, type Group, type Range, type Sort } from './filters'

type Update = (patch: Partial<Record<keyof Filters, string | number | boolean | string[] | undefined>>, replace?: boolean) => void

function SearchBox({ value, onCommit, placeholder }: { value: string; onCommit: (q: string) => void; placeholder: string }) {
  const [draft, setDraft] = useState(value)
  const composing = useRef(false)
  const ref = useRef<HTMLInputElement>(null)
  useEffect(() => setDraft(value), [value])
  useEffect(() => {
    if (composing.current || draft === value) return
    const t = setTimeout(() => onCommit(draft), 280)
    return () => clearTimeout(t)
  }, [draft, value, onCommit])
  useEffect(() => {
    // "/" always focuses this box; ⌘F only when no conversation is open (then ⌘F finds inside it).
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLElement && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.isContentEditable)
      const slash = e.key === '/' && !typing && !e.metaKey && !e.ctrlKey
      const cmdF = (e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'f' && !document.querySelector('[data-reader]')
      if (slash || cmdF) {
        e.preventDefault()
        ref.current?.focus()
        ref.current?.select()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])
  return (
    <label className="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg border bg-background/60 px-2.5 shadow-xs focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/30">
      <Search className="size-4 shrink-0 text-muted-foreground" />
      <input
        ref={ref}
        value={draft}
        placeholder={placeholder}
        aria-label={placeholder}
        className="min-w-0 flex-1 bg-transparent text-[13.5px] outline-none placeholder:text-muted-foreground"
        onChange={(e) => setDraft(e.target.value)}
        onCompositionStart={() => (composing.current = true)}
        onCompositionEnd={(e) => {
          composing.current = false
          setDraft(e.currentTarget.value)
        }}
        onKeyDown={(e) => {
          if (e.nativeEvent.isComposing) return
          if (e.key === 'Enter') onCommit(draft)
          if (e.key === 'Escape') {
            setDraft('')
            onCommit('')
          }
        }}
      />
      {draft ? (
        <button
          type="button"
          aria-label="清除搜尋"
          onClick={() => {
            setDraft('')
            onCommit('')
          }}
          className="text-muted-foreground hover:text-foreground"
        >
          <X className="size-3.5" />
        </button>
      ) : (
        <Kbd>/</Kbd>
      )}
    </label>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5 px-3 py-2.5">
      <span className="text-[11.5px] font-medium text-muted-foreground">{title}</span>
      {children}
    </div>
  )
}

function Pill({ on, onClick, children }: { on: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onClick}
      className={cn('flex h-7 items-center gap-1.5 rounded-md border px-2 text-[12.5px] whitespace-nowrap', on ? 'border-brand/40 bg-brand-soft' : 'text-muted-foreground hover:bg-muted hover:text-foreground')}
    >
      {children}
    </button>
  )
}

function ProjectField({ value, onChange }: { value?: string; onChange: (id?: string) => void }) {
  const [open, setOpen] = useState(false)
  const current = value ? archive.projectById.get(value) : undefined
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button type="button" className="flex h-8 items-center gap-1.5 rounded-md border px-2 text-left text-[13px]">
          {current ? <ProjectIcon project={current} /> : null}
          <span className={cn('flex-1 truncate', !current && 'text-muted-foreground')}>{current ? current.name : `全部 ${num(projectsByRecent.length)} 個專案`}</span>
          <ChevronsUpDown className="size-3.5 text-muted-foreground" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-[320px] p-0">
        <Command>
          <CommandInput placeholder="找專案…" />
          <CommandList className="max-h-[320px]">
            <CommandEmpty>沒有這個專案</CommandEmpty>
            <CommandGroup>
              {current && (
                <CommandItem value="__all" onSelect={() => { onChange(undefined); setOpen(false) }}>
                  全部專案
                </CommandItem>
              )}
              {projectsByRecent.map((p) => (
                <CommandItem key={p.id} value={`${p.name} ${p.locations[0].path}`} onSelect={() => { onChange(p.id); setOpen(false) }}>
                  <ProjectIcon project={p} />
                  <span className="truncate">{p.name}</span>
                  <span className="ml-auto shrink-0 text-xs text-muted-foreground">{ago(p.lastActive)}</span>
                  {p.id === value && <Check className="size-3.5" />}
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  )
}

export function Toolbar({ filters, update, scoped }: { filters: Filters; update: Update; scoped?: boolean }) {
  const toggleHarness = (h: HarnessId) => update({ harnesses: filters.harnesses.includes(h) ? filters.harnesses.filter((x) => x !== h) : [...filters.harnesses, h] })
  const toggleFlag = (f: Flag) => update({ flags: filters.flags.includes(f) ? filters.flags.filter((x) => x !== f) : [...filters.flags, f] })
  const project = !scoped && filters.project ? archive.projectById.get(filters.project) : undefined
  const active = (project ? 1 : 0) + filters.harnesses.length + (filters.range !== 'all' ? 1 : 0) + filters.flags.length
  const chips: { key: string; label: ReactNode; clear: () => void }[] = []
  if (project) chips.push({ key: 'p', label: project.name, clear: () => update({ project: undefined }) })
  for (const h of filters.harnesses) chips.push({ key: h, label: harnesses[h].name, clear: () => toggleHarness(h) })
  if (filters.range !== 'all') chips.push({ key: 'r', label: rangeLabels[filters.range], clear: () => update({ range: 'all' }) })
  for (const f of filters.flags) chips.push({ key: f, label: flagLabels[f], clear: () => toggleFlag(f) })

  return (
    <div className="flex flex-col gap-2 px-3 pt-3 pb-2">
      <div className="flex items-center gap-1.5">
        <SearchBox value={filters.q} placeholder={scoped ? '搜尋這個專案的對話…' : '搜尋所有對話…'} onCommit={(q) => update({ q, find: undefined, at: undefined }, false)} />
        <Popover>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={active ? `篩選（${active} 個條件）` : '篩選與排序'}
              className={cn('relative flex h-9 shrink-0 items-center gap-1.5 rounded-lg border px-2.5 text-[13px] text-muted-foreground hover:bg-muted hover:text-foreground', active > 0 && 'border-brand/40 text-foreground')}
            >
              <SlidersHorizontal className="size-4" />
              {active > 0 && <span className="flex size-4 items-center justify-center rounded-full bg-brand text-[10px] font-semibold text-white tabular">{active}</span>}
            </button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-[340px] p-0">
            <div className="flex flex-col divide-y">
              {!scoped && (
                <Section title="專案">
                  <ProjectField value={filters.project} onChange={(p) => update({ project: p })} />
                </Section>
              )}
              <Section title="工具">
                <div className="flex flex-wrap gap-1.5">
                  <Pill on={filters.harnesses.length === 0} onClick={() => update({ harnesses: [] })}>
                    全部
                  </Pill>
                  {harnessOrder.map((h) => (
                    <Pill key={h} on={filters.harnesses.includes(h)} onClick={() => toggleHarness(h)}>
                      <HarnessIcon id={h} className="size-3.5 [&_img]:size-3.5" />
                      {harnesses[h].name}
                    </Pill>
                  ))}
                </div>
              </Section>
              <Section title="時間">
                <div className="flex flex-wrap gap-1.5">
                  {(Object.keys(rangeLabels) as Range[]).map((r) => (
                    <Pill key={r} on={filters.range === r} onClick={() => update({ range: r })}>
                      {rangeLabels[r]}
                    </Pill>
                  ))}
                </div>
              </Section>
              <Section title="只看">
                <div className="flex flex-wrap gap-1.5">
                  {(Object.keys(flagLabels) as Flag[]).map((f) => (
                    <Pill key={f} on={filters.flags.includes(f)} onClick={() => toggleFlag(f)}>
                      {flagLabels[f]}
                    </Pill>
                  ))}
                </div>
              </Section>
              {!filters.q && (
                <Section title="排序">
                  <div className="flex flex-wrap gap-1.5">
                    {(Object.keys(sortLabels) as Sort[]).map((s) => (
                      <Pill key={s} on={filters.sort === s} onClick={() => update({ sort: s, group: s === 'recent' || s === 'started' ? filters.group : filters.group === 'day' ? 'none' : filters.group })}>
                        {sortLabels[s]}
                      </Pill>
                    ))}
                  </div>
                </Section>
              )}
              {!filters.q && (
                <Section title="分組">
                  <div className="flex flex-wrap gap-1.5">
                    {(Object.keys(groupLabels) as Group[])
                      .filter((g) => !(scoped && g === 'project'))
                      .map((g) => (
                        <Pill key={g} on={filters.group === g} onClick={() => update({ group: g })}>
                          {groupLabels[g]}
                        </Pill>
                      ))}
                  </div>
                </Section>
              )}
            </div>
          </PopoverContent>
        </Popover>
      </div>
      {chips.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          {chips.map((c) => (
            <span key={c.key} className="flex h-6 items-center gap-1 rounded-md bg-brand-soft pr-1 pl-2 text-[12px]">
              {c.label}
              <button type="button" aria-label="移除這個條件" onClick={c.clear} className="text-muted-foreground hover:text-foreground">
                <X className="size-3" />
              </button>
            </span>
          ))}
          {chips.length > 1 && (
            <button type="button" className="px-1 text-[12px] text-muted-foreground hover:text-foreground" onClick={() => update({ project: scoped ? filters.project : undefined, harnesses: [], range: 'all', flags: [] })}>
              全部清除
            </button>
          )}
        </div>
      )}
    </div>
  )
}
