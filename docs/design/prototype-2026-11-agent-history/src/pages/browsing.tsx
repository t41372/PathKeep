/**
 * Browsing history, a look-alike of today's History screen (toolbar, day
 * groups, visit rows) so the new rail and tabs can be judged with the
 * browsing type on. Not redesigned here.
 */
import { CalendarDays, Globe, Search, Star } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { dayLabel, localDayKey, time } from '~/data/format'
import { browserSources } from './backup/sources-data'
import { visits, type Visit } from './browsing/visits'

function Favicon({ domain }: { domain: string }) {
  let h = 0
  for (const ch of domain) h = (h * 31 + ch.charCodeAt(0)) % 360
  return (
    <span
      aria-hidden
      className="flex size-6 shrink-0 items-center justify-center rounded-md text-[11px] font-semibold text-white"
      style={{ background: `oklch(0.62 0.12 ${h})` }}
    >
      {domain.replace(/^www\./, '')[0].toUpperCase()}
    </span>
  )
}

const iconOf = Object.fromEntries(browserSources.map((b) => [b.name, b.icon]))

function VisitRow({ visit }: { visit: Visit }) {
  return (
    <li className="flex items-center gap-3 rounded-lg px-3 py-2 hover:bg-foreground/[0.035]">
      <span className="w-11 shrink-0 font-mono text-[12px] text-muted-foreground tabular">{time(visit.at)}</span>
      <Favicon domain={visit.domain} />
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-[13.5px]">{visit.title}</span>
        <span className="truncate text-xs text-muted-foreground">
          {visit.domain}
          <span className="opacity-60">{visit.path}</span>
        </span>
      </span>
      {visit.starred && <Star className="size-3.5 fill-brand text-brand" aria-label="已加星" />}
      <img src={iconOf[visit.browser]} alt={visit.browser} title={visit.browser} className="size-4" />
    </li>
  )
}

export default function BrowsingHistoryPage() {
  const [q, setQ] = useState('')
  const [view, setView] = useState('timeline')
  const groups = useMemo(() => {
    const needle = q.trim().toLowerCase()
    const list = visits.filter((v) => (!needle || `${v.title} ${v.domain}`.toLowerCase().includes(needle)) && (view !== 'starred' || v.starred))
    const map = new Map<string, Visit[]>()
    for (const v of list) {
      // 網站 groups by site, the other views by day.
      const key = view === 'sites' ? v.domain : localDayKey(v.at)
      map.set(key, [...(map.get(key) ?? []), v])
    }
    return [...map.entries()]
  }, [q, view])

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="flex shrink-0 items-center gap-2.5 border-b px-5 py-3">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="搜尋標題、網址、筆記" className="pl-9" aria-label="搜尋瀏覽紀錄" />
        </div>
        <Button variant="outline" size="sm">
          <CalendarDays />
          日期
        </Button>
        <Button variant="outline" size="sm">
          <Globe />
          所有瀏覽器
        </Button>
        <ToggleGroup type="single" variant="outline" size="sm" value={view} onValueChange={(v) => v && setView(v)}>
          <ToggleGroupItem value="timeline" className="px-3">
            時間線
          </ToggleGroupItem>
          <ToggleGroupItem value="sites" className="px-3">
            網站
          </ToggleGroupItem>
          <ToggleGroupItem value="starred" className="px-3">
            已加星
          </ToggleGroupItem>
        </ToggleGroup>
      </div>
      <div className="thin-scroll flex-1 overflow-y-auto px-3 pb-8">
        {groups.length === 0 && <p className="px-3 py-10 text-center text-[13px] text-muted-foreground">沒有符合的紀錄</p>}
        {groups.map(([key, list]) => (
          <section key={key}>
            <h2 className="sticky top-0 z-10 bg-panel/95 px-3 pt-4 pb-1.5 text-[12px] font-semibold text-muted-foreground backdrop-blur">
              {view === 'sites' ? key : dayLabel(key)} <span className="font-normal">· {list.length} 筆</span>
            </h2>
            <ul>
              {list.map((v) => (
                <VisitRow key={v.id} visit={v} />
              ))}
            </ul>
          </section>
        ))}
      </div>
    </div>
  )
}
