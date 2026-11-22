/**
 * Settings → 一般 → 洞察的時區. Mirrors `src/features/settings/insights-zone-row.tsx`
 * and the notice it holds (`src/features/insights/zone-notice.tsx`): every zone
 * this window knows, searched by city, zone name or IANA name; choosing
 * another one "rebuilds" Insights for a few seconds, and while the computer
 * is in another zone (the prototype switch) the row shows the notice until
 * a zone is chosen or kept.
 */
import { Check, ChevronsUpDown, Globe } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Command, CommandEmpty, CommandInput, CommandItem, CommandList } from '@/components/ui/command'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { hasZoneLabel, knownTimeZones, zoneLabel } from '@/lib/time-zone'

const locale = 'zh-TW'
/** The prototype's Insights zone, and the computer's zone when the switch says it moved. */
export const INSIGHTS_ZONE = 'Asia/Taipei'
export const TRAVEL_ZONE = 'America/Los_Angeles'

/** `zoneText` in `features/insights/zone.ts`: its name and offset, with the place when two names clash. */
function zoneText(zone: string, others: string[] = []) {
  const label = zoneLabel(zone, locale)
  const name = label.name || label.place
  const clash = others.some((other) => {
    const theirs = zoneLabel(other, locale)
    return other !== zone && (theirs.name || theirs.place) === name
  })
  return clash && label.name ? `${name}・${label.place}（${label.offset}）` : `${name}（${label.offset}）`
}

/** Labels the zones a few at a time, as the app does, so opening the list has names without a long task. */
function useZoneLabels(zones: string[]) {
  const [, setLabelled] = useState(0)
  const ready = zones.every((zone) => hasZoneLabel(zone, locale))
  useEffect(() => {
    if (ready) return
    let index = 0
    let timer = 0
    const step = () => {
      const until = performance.now() + 6
      while (index < zones.length && performance.now() < until) {
        zoneLabel(zones[index], locale)
        index += 1
      }
      if (index < zones.length) timer = window.setTimeout(step, 0)
      else setLabelled((n) => n + 1)
    }
    timer = window.setTimeout(step, 0)
    return () => window.clearTimeout(timer)
  }, [zones, ready])
  return ready
}

export function InsightsZoneRow({ computer }: { computer: string }) {
  const [shown, setShown] = useState(INSIGHTS_ZONE)
  const [next, setNext] = useState(INSIGHTS_ZONE)
  const [saving, setSaving] = useState(false)
  const [kept, setKept] = useState<string | null>(null)
  const [open, setOpen] = useState(false)
  const zones = useMemo(() => knownTimeZones([next, shown, computer]).sort(), [next, shown, computer])
  const labelled = useZoneLabels(zones)
  const current = zoneLabel(next, locale)
  const rebuilding = next !== shown

  // The rebuild finishes after a few seconds.
  useEffect(() => {
    if (!rebuilding) return
    const timer = setTimeout(() => setShown(next), 4000)
    return () => clearTimeout(timer)
  }, [rebuilding, next])

  // Choosing or keeping a zone remembers the computer's, so the notice stays away until it moves again.
  const choose = (zone: string) => {
    setSaving(true)
    setTimeout(() => {
      setSaving(false)
      setNext(zone)
      setKept(computer)
    }, 400)
  }

  const mismatch = computer !== next && kept !== computer

  return (
    <SettingRow
      title="洞察的時區"
      htmlFor="settings-insights-zone"
      description={
        <>
          洞察按這個時區把每次瀏覽算進某一天、某個小時。出門旅行時它不跟著變；改了會重新整理洞察。
          {rebuilding && <span className="mt-1 block text-foreground">正在按{zoneText(next, [shown])}重新整理洞察，完成之前仍是{zoneText(shown, [next])}。</span>}
        </>
      }
      control={
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild>
            <Button id="settings-insights-zone" variant="outline" size="sm" role="combobox" aria-expanded={open} disabled={saving} className="min-w-[160px] justify-between bg-card text-[13px] font-normal">
              <span className="truncate">
                {current.place}
                <span className="ml-1.5 text-muted-foreground tabular">{current.offset}</span>
              </span>
              <ChevronsUpDown className="size-3.5 opacity-50" aria-hidden />
            </Button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-[320px] p-0">
            <Command>
              <CommandInput placeholder="搜尋城市或時區" />
              <CommandList>
                <CommandEmpty>沒有符合的時區。</CommandEmpty>
                {zones.map((id) => {
                  const label = labelled ? zoneLabel(id, locale) : null
                  const place = id.split('/').at(-1)?.replaceAll('_', ' ')
                  return (
                    <CommandItem
                      key={id}
                      value={id}
                      aria-label={label ? [place, label.name || id, label.offset].join(', ') : place}
                      keywords={[id.replaceAll('_', ' '), ...(label ? [label.name, label.offset] : [])]}
                      onSelect={() => {
                        setOpen(false)
                        if (id !== next) choose(id)
                      }}
                      className="text-[13px]"
                    >
                      <Check className={id === next ? 'size-3.5' : 'size-3.5 opacity-0'} aria-hidden />
                      <span className="flex min-w-0 flex-1 flex-col">
                        <span className="truncate">{place}</span>
                        <span className="truncate text-xs text-muted-foreground">{label?.name || id}</span>
                      </span>
                      <span className="shrink-0 text-xs text-muted-foreground tabular">{label?.offset}</span>
                    </CommandItem>
                  )
                })}
              </CommandList>
            </Command>
          </PopoverContent>
        </Popover>
      }
    >
      {mismatch && <ZoneNotice insightsZone={next} computer={computer} saving={saving} onKeep={() => choose(next)} onUse={() => choose(computer)} />}
    </SettingRow>
  )
}

/** `features/insights/zone-notice.tsx`: the computer is in another zone than Insights. */
function ZoneNotice({ insightsZone, computer, saving, onKeep, onUse }: { insightsZone: string; computer: string; saving: boolean; onKeep: () => void; onUse: () => void }) {
  const both = [insightsZone, computer]
  const insights = zoneText(insightsZone, both)
  const there = zoneText(computer, both)
  return (
    <div role="status" className="flex flex-col gap-3 rounded-xl bg-brand-soft p-3.5 text-[13px]">
      <div className="flex items-start gap-2.5">
        <Globe className="mt-0.5 size-4 shrink-0 text-brand" aria-hidden />
        <div className="flex flex-col gap-0.5">
          <p className="font-medium">
            洞察用的是{insights}，這台電腦現在是{there}。
          </p>
          <p className="text-muted-foreground">不切換的話，洞察裡的日子與時刻維持不變；切換會重新整理洞察。</p>
        </div>
      </div>
      <div className="flex flex-wrap justify-end gap-2">
        <Button size="sm" variant="outline" disabled={saving} onClick={onKeep}>
          保持{insights}
        </Button>
        <Button size="sm" disabled={saving} onClick={onUse}>
          {saving ? '正在儲存…' : `改用${there}`}
        </Button>
      </div>
    </div>
  )
}
