/**
 * The browsing space's Home cards: today's app Home, trimmed. Numbers are
 * invented; browsing is not rebuilt in this prototype.
 */
import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { SectionCard } from '@/components/app/section-card'
import { EvilAreaChart } from '@/components/evilcharts/charts/recharts-area-chart'
import { DAY, NOW } from '~/data/fixture'
import { date, num } from '~/data/format'
import { mulberry32 } from '~/data/rng'
import { YearGrid, YearStepper } from '../agent/insights/parts'
import { invented } from '../home/data'
import { visits } from './visits'

const trend = (() => {
  const rng = mulberry32(31)
  return Array.from({ length: 30 }, (_, i) => {
    const t = NOW - (29 - i) * DAY
    const d = new Date(t + 8 * 3600_000)
    const weekend = d.getUTCDay() === 0 || d.getUTCDay() === 6
    return { day: d.toISOString().slice(0, 10), visits: Math.round((weekend ? 140 : 320) + rng() * 220) }
  })
})()

const config = { visits: { label: '造訪', colors: { light: ['var(--brand)'], dark: ['var(--brand)'] } } }

export function TrendCard() {
  const total = trend.reduce((s, d) => s + d.visits, 0)
  return (
    <SectionCard title="最近 30 天" subtitle={`每天的造訪，共 ${num(total)} 筆`}>
      <div className="h-[200px]">
        <EvilAreaChart data={trend} config={config} xDataKey="day" className="aspect-auto h-full" curveType="monotone">
          <defs>
            <linearGradient id="browse-trend-fill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--brand)" stopOpacity={0.35} />
              <stop offset="100%" stopColor="var(--brand)" stopOpacity={0} />
            </linearGradient>
          </defs>
          <EvilAreaChart.Grid vertical={false} strokeDasharray="3 4" />
          <EvilAreaChart.XAxis
            dataKey="day"
            tickLine={false}
            axisLine={false}
            interval={6}
            padding={{ left: 14, right: 14 }}
            tickFormatter={(v: string) => `${Number(v.slice(5, 7))}/${Number(v.slice(8, 10))}`}
            className="font-mono"
          />
          <EvilAreaChart.Tooltip />
          <EvilAreaChart.Area dataKey="visits" variant="gradient" strokeVariant="solid" strokeWidth={2} areaProps={{ dataKey: 'visits', fill: 'url(#browse-trend-fill)' }}>
            <EvilAreaChart.ActiveDot variant="colored-border" />
          </EvilAreaChart.Area>
        </EvilAreaChart>
      </div>
    </SectionCard>
  )
}

const earlier = [
  { year: 2025, pages: 214, top: 'github.com、docs.rs', sample: visits.slice(3, 6) },
  { year: 2024, pages: 168, top: 'www.youtube.com、zh.wikipedia.org', sample: visits.slice(9, 11) },
]

export function OnThisDayCard() {
  return (
    <SectionCard title="往年今日" subtitle="以前的 11 月 13 日，你在看什麼">
      <div className="flex flex-col gap-3">
        {earlier.map((e) => (
          <div key={e.year} className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              {date(Date.UTC(e.year, 10, 13), true)} · {num(e.pages)} 頁，最多在 {e.top}
            </span>
            <ul className="flex flex-col gap-0.5">
              {e.sample.map((v) => (
                <li key={v.id}>
                  <Link to="/browsing/history" className="flex min-w-0 items-center gap-2 rounded-md px-1.5 py-1 text-[13px] hover:bg-foreground/[0.035]">
                    <span className="flex size-4 shrink-0 items-center justify-center rounded bg-muted text-[9px] font-semibold uppercase">{v.domain.replace('www.', '')[0]}</span>
                    <span className="truncate">{v.title}</span>
                  </Link>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </SectionCard>
  )
}

export function YearCard() {
  const [year, setYear] = useState(2026)
  const counts = useMemo(() => invented('browsing', year), [year])
  const label = useMemo(() => (key: string, v: number) => (v > 0 ? `${key} · ${num(v)} 頁` : `${key} · 沒有`), [])
  const sum = [...counts.values()].reduce((s, v) => s + v, 0)
  return (
    <SectionCard title="這一年" subtitle="每天瀏覽的頁數" action={<YearStepper year={year} min={2025} max={2026} onChange={setYear} />}>
      <YearGrid year={year} counts={counts} label={label} summary={`${year} 年共 ${num(sum)} 頁`} />
    </SectionCard>
  )
}
