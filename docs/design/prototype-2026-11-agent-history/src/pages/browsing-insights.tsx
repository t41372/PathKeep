/** Browsing insights, a look-alike of today's Insights head: KPI row, daily bars, top sites. */
import { PageHeader, PageScroll, SectionCard } from '@/components/app/section-card'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { Stat } from '~/components/bits'
import { DAY, NOW } from '~/data/fixture'
import { localDayKey, num } from '~/data/format'
import { mulberry32 } from '~/data/rng'
import { NotRedrawn } from '~/components/not-redrawn'

const rng = mulberry32(7)
const daily = Array.from({ length: 30 }, (_, i) => {
  const t = NOW - (29 - i) * DAY
  const weekend = [0, 6].includes(new Date(t + 8 * 3600_000).getUTCDay())
  const pages = Math.round((weekend ? 180 : 420) * (0.7 + rng() * 0.6))
  return { dateKey: localDayKey(t), pages, searches: Math.round(pages * (0.12 + rng() * 0.06)) }
})

const config = {
  pages: { label: '頁面', colors: { light: ['var(--brand)'], dark: ['var(--brand)'] } },
  searches: { label: '搜尋', colors: { light: ['var(--blue)'], dark: ['var(--blue)'] } },
}

const topSites = [
  ['github.com', 2140],
  ['docs.rs', 1312],
  ['www.youtube.com', 988],
  ['developer.mozilla.org', 640],
  ['news.ycombinator.com', 512],
  ['mail.google.com', 470],
  ['stackoverflow.com', 388],
] as const

export default function BrowsingInsightsPage() {
  const total = daily.reduce((s, d) => s + d.pages, 0)
  const searches = daily.reduce((s, d) => s + d.searches, 0)
  const max = topSites[0][1]
  return (
    <PageScroll wide>
      <NotRedrawn page="洞察" />
      <PageHeader title="洞察" subtitle="瀏覽紀錄 · 最近 30 天" />
      <div className="grid grid-cols-[repeat(auto-fit,minmax(180px,1fr))] gap-3">
        <Stat label="造訪的頁面" value={num(total)} sub={`每天平均 ${num(total / 30)} 頁`} />
        <Stat label="不同的網站" value="1,032" sub="其中 214 個第一次去" />
        <Stat label="搜尋" value={num(searches)} sub="Google 占 81%" />
        <Stat label="最常在線的時段" value="21–23 點" sub="週二最忙" />
      </div>
      <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <SectionCard
          title="每天的頁面與搜尋"
          action={
            <span className="flex items-center gap-3 text-xs text-muted-foreground">
              <span className="flex items-center gap-1.5">
                <span className="size-2 rounded-[2px] bg-brand" />
                頁面
              </span>
              <span className="flex items-center gap-1.5">
                <span className="size-2 rounded-[2px] bg-blue" />
                搜尋
              </span>
            </span>
          }
        >
          <div className="h-[220px]">
            <EvilBarChart data={daily} config={config} xDataKey="dateKey" className="aspect-auto h-full" barRadius={3} barGap={2}>
              <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
              <EvilBarChart.XAxis dataKey="dateKey" tickLine={false} axisLine={false} minTickGap={32} tickFormatter={(v: string) => `${Number(v.slice(5, 7))}/${Number(v.slice(8))}`} className="font-mono" />
              <EvilBarChart.Tooltip />
              <EvilBarChart.Bar dataKey="pages" />
              <EvilBarChart.Bar dataKey="searches" />
            </EvilBarChart>
          </div>
        </SectionCard>
        <SectionCard title="最常去的網站">
          <ul className="flex flex-col gap-2.5">
            {topSites.map(([domain, n]) => (
              <li key={domain} className="flex flex-col gap-1">
                <span className="flex justify-between text-[13px]">
                  <span className="truncate">{domain}</span>
                  <span className="font-mono text-muted-foreground tabular">{num(n)}</span>
                </span>
                <span className="h-1.5 overflow-hidden rounded-full bg-muted">
                  <span className="block h-full rounded-full bg-brand/70" style={{ width: `${(n / max) * 100}%` }} />
                </span>
              </li>
            ))}
          </ul>
        </SectionCard>
      </div>
    </PageScroll>
  )
}
