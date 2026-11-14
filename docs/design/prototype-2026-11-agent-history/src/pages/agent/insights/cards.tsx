/** Agent insights cards. Every card names what it counts and over which window. */
import { timeWords } from '~/data/labels'
import { useProto } from '~/proto-state'
import { costOf, PRICE_TABLE_VERSION } from '~/data/pricing'
import { ArrowRight, MessagesSquare } from 'lucide-react'
import { useMemo, useState, type ReactNode } from 'react'
import { DAY, NOW } from '~/data/fixture'
import { Link, useNavigate } from 'react-router-dom'
import { SectionCard } from '@/components/app/section-card'
import { Heatmap, HeatLegend, type HeatCell } from '@/components/app/heatmap'
import { heatLevel } from '@/components/app/heat-level'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { cn } from '@/lib/cn'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Coverage, HarnessBadge, ProjectIcon, Stat, TokenBar } from '~/components/bits'
import { archive, harnesses, projectOf } from '~/data/archive'
import { totalTokens } from '~/data/fixture'
import { big, duration, hours, num, pct, usd } from '~/data/format'
import {
  bucketLabel,
  bucketWord,
  dailyAgentHours,
  rangeWindow,
  type Insights,
  type Scope,
} from './data'
import { HoursWithEquivalent } from './equivalents'
import { harnessColor, Legend, ShareList, YearGrid, YearStepper } from './parts'

// ------------------------------------------------------------------ KPIs

export function KpiRow({ ins }: { ins: Insights }) {
  const cacheShare = ins.tokens.cacheRead / Math.max(1, totalTokens(ins.tokens))
  const total = ins.chats.length
  const cost = costOf(ins.chats)
  const { estimateCost } = useProto()
  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(190px,1fr))] gap-3">
      <Stat
        label={timeWords.agent}
        value={<HoursWithEquivalent ms={ins.busy}>{hours(ins.busy)}</HoursWithEquivalent>}
        sub={
          <>
            {timeWords.summed} {hours(ins.agentMs)} · {timeWords.concurrency(ins.concurrency)}
          </>
        }
        hint={
          <>
            {timeWords.agentHow}
            <br />
            {timeWords.summed}：{timeWords.summedHow}
            <br />
            {num(ins.recordedDurations)} / {num(total)} 段的時長是工具自己記的，其餘從時間戳推算。
          </>
        }
      />
      <Stat
        label={timeWords.present}
        value={<HoursWithEquivalent ms={ins.present}>{hours(ins.present)}</HoursWithEquivalent>}
        sub="估算：你在跟 Agent 來回的時段"
        hint={timeWords.presentHow}
      />
      <Stat
        label="Token"
        value={big(totalTokens(ins.tokens))}
        sub={ins.tokensKnown < total ? <Coverage known={ins.tokensKnown} total={total} what="用量" /> : `其中 ${pct(cacheShare)} 是讀取快取`}
        hint="輸入、寫入快取、讀取快取、輸出四種加起來。每一輪都會把前面的對話重新送給模型，大多從快取讀，所以讀取快取占了絕大部分。"
      />
      <Stat
        label="費用（按 API 牌價）"
        value={usd(estimateCost ? cost.total : cost.recorded)}
        sub={estimateCost ? `工具記下 ${usd(cost.recorded)} · 估算 ${usd(cost.estimated)}` : `只算工具記下的（${num(cost.recordedChats)} 段）`}
        hint="訂閱方案不按 token 收費，這是換成 API 牌價的等值金額。工具自己記了金額的用它記的，其餘按 PathKeep 附的牌價表估算。"
      />
      <Stat label="對話" value={num(total)} sub={`另有 ${num(ins.subagents)} 個 subagent · ${num(ins.projectCount)} 個專案`} to="/agent/chats" />
    </div>
  )
}

/** The page's one chart: agent time or tokens over the range. */
export function WorkCard({ ins, scope }: { ins: Insights; scope: Scope }) {
  const [metric, setMetric] = useState<'time' | 'tokens'>('time')
  const [withRead, setWithRead] = useState(false)
  const hoursData = useMemo(
    () => ins.hoursSeries.map((r) => ({ k: r.k, busy: Math.round(Number(r.busy) * 10) / 10, summed: Math.round(Number(r.summed) * 10) / 10 })),
    [ins],
  )
  const chart = useMemo(() => tokenChart(ins, 'kind', withRead), [ins, withRead])
  const unit = metric === 'time' ? '小時' : chart.unit ? `${chart.unit} token` : 'token'
  return (
    <SectionCard
      title={metric === 'time' ? 'Agent 工作的時間' : 'Token 用量'}
      subtitle={`${rangeWindow[scope.range]}${bucketWord[ins.bucket]}，單位：${unit}`}
      action={
        <ToggleGroup type="single" size="sm" variant="outline" value={metric} onValueChange={(v) => v && setMetric(v as 'time' | 'tokens')}>
          <ToggleGroupItem value="time" className="px-2.5 text-xs">時間</ToggleGroupItem>
          <ToggleGroupItem value="tokens" className="px-2.5 text-xs">Token</ToggleGroupItem>
        </ToggleGroup>
      }
    >
      <div className="flex min-h-6 flex-wrap items-center justify-between gap-3">
        {metric === 'time' ? (
          <Legend
            items={[
              { label: `${timeWords.agent}（同時開的只算一次）`, color: 'var(--brand)' },
              { label: `${timeWords.summed}（同時開的各算各的）`, color: 'color-mix(in oklab, var(--muted-foreground) 40%, transparent)' },
            ]}
          />
        ) : (
          <>
            <Legend items={chart.series.map((s) => ({ label: s.label, color: s.color }))} />
            <label className="flex items-center gap-2 text-xs text-muted-foreground">
              含讀取快取
              <Switch checked={withRead} onCheckedChange={setWithRead} />
            </label>
          </>
        )}
      </div>
      <div className="h-[230px]">
        {metric === 'time' ? (
          <EvilBarChart key="time" data={hoursData} config={hourConfig} xDataKey="k" className="aspect-auto h-full" barRadius={2} barGap={1}>
            <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
            <EvilBarChart.XAxis dataKey="k" tickLine={false} axisLine={false} minTickGap={28} tickFormatter={(v: string) => bucketLabel(v, ins.bucket)} className="font-mono" />
            <EvilBarChart.YAxis tickFormatter={(v: number) => `${v}`} className="font-mono" width={36} />
            <EvilBarChart.Tooltip />
            <EvilBarChart.Bar dataKey="busy" />
            <EvilBarChart.Bar dataKey="summed" />
          </EvilBarChart>
        ) : (
          <EvilBarChart key={`tokens-${withRead}`} data={chart.data} config={chart.config} xDataKey="k" stackType="stacked" className="aspect-auto h-full" barRadius={2}>
            <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
            <EvilBarChart.XAxis dataKey="k" tickLine={false} axisLine={false} minTickGap={28} tickFormatter={(v: string) => bucketLabel(v, ins.bucket)} className="font-mono" />
            <EvilBarChart.YAxis tickFormatter={(v: number) => `${num(v)}`} className="font-mono" width={44} />
            <EvilBarChart.Tooltip />
            {chart.series.map((_, i) => (
              <EvilBarChart.Bar key={i} dataKey={`s${i}`} />
            ))}
          </EvilBarChart>
        )}
      </div>
    </SectionCard>
  )
}

/** Time you were there, an estimate, next to how the agents' time splits by tool. */
export function PresenceCard({ ins }: { ins: Insights }) {
  return (
    <SectionCard title={`${timeWords.present}（估算）`} subtitle={timeWords.presentHow}>
      <div className="flex items-baseline gap-2">
        <span className="text-2xl font-semibold tracking-[-0.02em] tabular">{hours(ins.present)}</span>
        <span className="text-xs text-muted-foreground">同一範圍裡 {timeWords.agent} {hours(ins.busy)}</span>
      </div>
      <p className="text-[13px] leading-relaxed text-muted-foreground">這不是打卡紀錄。Agent 自己跑長任務時你可能不在，所以兩個數字不會一樣。</p>
    </SectionCard>
  )
}

export function ChangesCard({ ins }: { ins: Insights }) {
  const touched = ins.chats.filter((c) => c.linesAdded > 0).length
  return (
    <SectionCard title="改動的程式碼" subtitle="Agent 用編輯工具改檔案時記下的行數，不含它用 shell 指令改的部分">
      <div className="flex items-baseline gap-3">
        <span className="text-2xl font-semibold tracking-[-0.02em] text-green tabular">+{big(ins.linesAdded)}</span>
        <span className="text-lg font-medium text-red tabular">−{big(ins.linesRemoved)}</span>
        <span className="text-xs text-muted-foreground">{num(touched)} / {num(ins.chats.length)} 段改過檔案</span>
      </div>
    </SectionCard>
  )
}

// ---------------------------------------------------------------- tokens

type Split = 'kind' | 'harness' | 'model'

const kindSeries = [
  { key: 'output', label: '輸出', color: 'var(--brand)' },
  { key: 'input', label: '輸入', color: 'var(--blue)' },
  { key: 'cacheWrite', label: '寫入快取', color: 'var(--violet)' },
  { key: 'cacheRead', label: '讀取快取', color: 'color-mix(in oklab, var(--muted-foreground) 55%, transparent)' },
]
const modelColors = ['var(--brand)', 'var(--blue)', 'var(--green)', 'var(--violet)', 'var(--red)', 'color-mix(in oklab, var(--muted-foreground) 55%, transparent)']

function tokenChart(ins: Insights, split: Split, withRead: boolean) {
  let series: { key: string; label: string; color: string; from: (row: Record<string, number | string>) => number }[]
  if (split === 'kind') {
    series = kindSeries
      .filter((s) => withRead || s.key !== 'cacheRead')
      .map((s) => ({ ...s, from: (row) => Number(row[s.key] ?? 0) }))
  } else if (split === 'harness') {
    series = ins.harnessMix.map((h) => ({
      key: h.id,
      label: harnesses[h.id].name,
      color: harnessColor[h.id],
      from: (row) => Number(row[withRead ? h.id : h.id + 'NoRead'] ?? 0),
    }))
  } else {
    const top = ins.models.filter((m) => m.tokens > 0).slice(0, 5)
    const rest = ins.models.filter((m) => m.tokens > 0).slice(5)
    series = top.map((m, i) => ({ key: m.key, label: m.model, color: modelColors[i], from: (row) => Number(row[withRead ? m.key : m.key + 'NoRead'] ?? 0) }))
    if (rest.length)
      series.push({
        key: 'other',
        label: `其他 ${rest.length} 個`,
        color: modelColors[5],
        from: (row) => rest.reduce((s, m) => s + Number(row[withRead ? m.key : m.key + 'NoRead'] ?? 0), 0),
      })
  }
  const source = split === 'kind' ? ins.tokenSeries : split === 'harness' ? ins.harnessTokenSeries : ins.modelTokenSeries
  const raw = source.map((row) => {
    const out: Record<string, number | string> = { k: row.k }
    series.forEach((s, i) => (out[`s${i}`] = s.from(row)))
    return out
  })
  const max = Math.max(1, ...raw.map((r) => series.reduce((s, _, i) => s + Number(r[`s${i}`]), 0)))
  const [div, unit] = max >= 1e8 ? [1e8, '億'] : max >= 1e4 ? [1e4, '萬'] : [1, '']
  const data = raw.map((r) => {
    const out: Record<string, number | string> = { k: r.k }
    series.forEach((_, i) => (out[`s${i}`] = Math.round((Number(r[`s${i}`]) / div) * 100) / 100))
    return out
  })
  const config = Object.fromEntries(series.map((s, i) => [`s${i}`, { label: s.label, colors: { light: [s.color], dark: [s.color] } }]))
  return { data, config, series, unit }
}

export function TokensCard({ ins, scope }: { ins: Insights; scope: Scope }) {
  const [split, setSplit] = useState<Split>('kind')
  const [withRead, setWithRead] = useState(false)
  const chart = useMemo(() => tokenChart(ins, split, withRead), [ins, split, withRead])
  return (
    <SectionCard
      title="Token 用量"
      subtitle={`${rangeWindow[scope.range]}${bucketWord[ins.bucket]}的 token，按對話開始的時間歸入${chart.unit ? `，單位：${chart.unit}` : ''}`}
      action={
        <ToggleGroup type="single" size="sm" variant="outline" value={split} onValueChange={(v) => v && setSplit(v as Split)}>
          <ToggleGroupItem value="kind" className="px-2 text-xs">按種類</ToggleGroupItem>
          <ToggleGroupItem value="harness" className="px-2 text-xs">按工具</ToggleGroupItem>
          <ToggleGroupItem value="model" className="px-2 text-xs">按模型</ToggleGroupItem>
        </ToggleGroup>
      }
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Legend items={chart.series.map((s) => ({ label: s.label, color: s.color }))} />
        <label className="flex items-center gap-2 text-xs text-muted-foreground">
          含讀取快取
          <Switch checked={withRead} onCheckedChange={setWithRead} />
        </label>
      </div>
      <div className="h-[220px]">
        <EvilBarChart
          key={`${split}-${withRead}`}
          data={chart.data}
          config={chart.config}
          xDataKey="k"
          stackType="stacked"
          className="aspect-auto h-full"
          barRadius={2}
        >
          <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
          <EvilBarChart.XAxis dataKey="k" tickLine={false} axisLine={false} minTickGap={28} tickFormatter={(v: string) => bucketLabel(v, ins.bucket)} className="font-mono" />
          <EvilBarChart.YAxis tickFormatter={(v: number) => `${num(v)}${chart.unit}`} className="font-mono" width={52} />
          <EvilBarChart.Tooltip />
          {chart.series.map((_, i) => (
            <EvilBarChart.Bar key={i} dataKey={`s${i}`} />
          ))}
        </EvilBarChart>
      </div>
    </SectionCard>
  )
}

export function TokenMixCard({ ins }: { ins: Insights }) {
  const total = totalTokens(ins.tokens)
  return (
    <SectionCard title="Token 組成" subtitle="同一個範圍裡，四種 token 各占多少">
      <TokenBar tokens={ins.tokens} />
      <p className="text-[13px] leading-relaxed text-muted-foreground">
        讀取快取占 <span className="font-medium text-foreground tabular">{pct(ins.tokens.cacheRead / Math.max(1, total))}</span>。Agent 每一輪都把前面的對話重新送一次，大多直接從快取讀，所以這個數字很大，但比一般輸入便宜很多。
      </p>
      <ShareList
        items={ins.harnessMix.map((h) => ({
          key: h.id,
          label: <HarnessBadge id={h.id} withName />,
          value: h.tokens,
          display: big(h.tokens),
          sub: `${num(h.chats)} 段`,
          color: harnessColor[h.id],
        }))}
      />
      {ins.tokensKnown < ins.chats.length && (
        <p className="text-xs text-muted-foreground">
          <Coverage known={ins.tokensKnown} total={ins.chats.length} what="用量" />。
          {ins.chats.some((c) => !c.tokens && c.harness === 'claude') ? '沒有紀錄的大多是 2025 年 8 月以前的 Claude Code 對話，' : ''}
          沒有紀錄的不算成 0。
        </p>
      )}
    </SectionCard>
  )
}

// ------------------------------------------------------------------ time

const hourConfig = {
  busy: { label: timeWords.agent, colors: { light: ['var(--brand)'], dark: ['var(--brand)'] } },
  summed: {
    label: timeWords.summed,
    colors: {
      light: ['color-mix(in oklab, var(--muted-foreground) 40%, transparent)'],
      dark: ['color-mix(in oklab, var(--muted-foreground) 45%, transparent)'],
    },
  },
}

export function HoursCard({ ins, scope }: { ins: Insights; scope: Scope }) {
  const data = useMemo(
    () => ins.hoursSeries.map((r) => ({ k: r.k, busy: Math.round(Number(r.busy) * 10) / 10, summed: Math.round(Number(r.summed) * 10) / 10 })),
    [ins],
  )
  return (
    <SectionCard
      title="Agent 工作時間"
      subtitle={`${rangeWindow[scope.range]}${bucketWord[ins.bucket]}的小時數。兩根柱子的差，就是同時跑好幾個 Agent 的部分`}
    >
      <Legend
        items={[
          { label: `${timeWords.agent}（同時開的只算一次）`, color: 'var(--brand)' },
          { label: `${timeWords.summed}（同時開的各算各的）`, color: 'color-mix(in oklab, var(--muted-foreground) 40%, transparent)' },
        ]}
      />
      <div className="h-[220px]">
        <EvilBarChart data={data} config={hourConfig} xDataKey="k" className="aspect-auto h-full" barRadius={2} barGap={1}>
          <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
          <EvilBarChart.XAxis dataKey="k" tickLine={false} axisLine={false} minTickGap={28} tickFormatter={(v: string) => bucketLabel(v, ins.bucket)} className="font-mono" />
          <EvilBarChart.YAxis tickFormatter={(v: number) => `${v} 時`} className="font-mono" width={48} />
          <EvilBarChart.Tooltip />
          <EvilBarChart.Bar dataKey="busy" />
          <EvilBarChart.Bar dataKey="summed" />
        </EvilBarChart>
      </div>
    </SectionCard>
  )
}

export function HarnessTimeCard({ ins }: { ins: Insights }) {
  return (
    <SectionCard title="各工具的工作時間" subtitle={`${timeWords.summed}：${timeWords.summedHow}`}>
      <ShareList
        items={ins.harnessMix.map((h) => ({
          key: h.id,
          label: <HarnessBadge id={h.id} withName />,
          value: h.agentMs,
          display: hours(h.agentMs),
          sub: `${num(h.chats)} 段`,
          color: harnessColor[h.id],
        }))}
      />
    </SectionCard>
  )
}

const weekdays = ['週一', '週二', '週三', '週四', '週五', '週六', '週日']

export function RhythmCard({ ins, scope }: { ins: Insights; scope: Scope }) {
  const { columns, peak } = useMemo(() => {
    const max = Math.max(...ins.rhythm.flat())
    let best = { h: 0, d: 0, v: 0 }
    const columns: HeatCell[][] = ins.rhythm.map((col, h) =>
      col.map((v, d) => {
        if (v > best.v) best = { h, d, v }
        return { key: `${h}-${d}`, level: heatLevel(v, max), label: `${weekdays[d]} ${h}:00–${h + 1}:00 · 約 ${num(v)} 次提問` }
      }),
    )
    return { columns, peak: best }
  }, [ins])
  return (
    <SectionCard title="作息" subtitle={`${rangeWindow[scope.range]}，你在一週裡哪些時段送出提示`}>
      <div className="overflow-x-auto">
        <Heatmap
          columns={columns}
          label="一週各時段送出的提示"
          rowLabels={weekdays}
          columnLabels={Array.from({ length: 24 }, (_, h) => (h % 6 === 0 || h === 23 ? `${h}` : null))}
        />
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
        <span>
          最常在 <span className="font-medium text-foreground">{weekdays[peak.d]} {peak.h}:00</span> 左右下指令
        </span>
        <HeatLegend less="少" more="多" />
      </div>
    </SectionCard>
  )
}

/**
 * The year calendar and that year's figures, side by side. The calendar card
 * is exactly as wide as its grid; the figures card takes the rest of the row
 * and wraps below it when there isn't room. Both follow the one year stepper
 * and ignore the page's time range, which is why they sit apart from the wall.
 */
export function YearSection({ scope }: { scope: Scope }) {
  const [year, setYear] = useState(2026)
  const navigate = useNavigate()
  const counts = useMemo(() => dailyAgentHours(year, { harnesses: scope.harnesses, projectId: scope.projectId }), [year, scope.harnesses, scope.projectId])
  const label = useMemo(() => (key: string, v: number) => (v > 0 ? `${key} · Agent 工作 ${v >= 1 ? `${v.toFixed(1)} 小時` : `${Math.round(v * 60)} 分`}` : `${key} · 沒有`), [])
  const openDay = (key: string) => void navigate(`/agent/chats?day=${key}`)
  return (
    <div className="flex flex-wrap items-stretch gap-4">
      <div className="flex w-fit max-w-full flex-col">
        <SectionCard
          title="每天的 Agent 工作時間"
          subtitle="一年裡的每一天，同時開的只算一次。不受上面的時間範圍影響"
          action={<YearStepper year={year} min={2025} max={2026} onChange={setYear} />}
          className="flex-1"
        >
          <YearGrid year={year} counts={counts} label={label} onDay={openDay} />
        </SectionCard>
      </div>
      <div className="flex min-w-[300px] flex-1 flex-col">
        <YearFactsCard year={year} counts={counts} onDay={openDay} />
      </div>
    </div>
  )
}

const monthOf = (key: string) => Number(key.slice(5, 7)) - 1
const dayText = (key: string) => `${Number(key.slice(5, 7))}月${Number(key.slice(8, 10))}日`

function yearFacts(year: number, counts: Map<string, number>) {
  const days = [...counts].filter(([, v]) => v > 0).sort(([a], [b]) => (a < b ? -1 : 1))
  const total = days.reduce((s, [, v]) => s + v, 0)
  const months = Array.from({ length: 12 }, () => 0)
  let busiest = { key: '', v: 0 }
  let streak = { len: 0, from: '', to: '' }
  let run = 0
  let runFrom = ''
  let prev = 0
  for (const [key, v] of days) {
    months[monthOf(key)] += v
    if (v > busiest.v) busiest = { key, v }
    const t = Date.parse(key)
    run = prev && t - prev === DAY ? run + 1 : 1
    if (run === 1) runFrom = key
    if (run > streak.len) streak = { len: run, from: runFrom, to: key }
    prev = t
  }
  const peakMonth = months.indexOf(Math.max(...months))
  // Days that have happened so far in that year, so the share reads right for the current one.
  const elapsed = year === new Date(NOW).getUTCFullYear() ? Math.floor((NOW - Date.UTC(year, 0, 1)) / DAY) + 1 : Math.round((Date.UTC(year + 1, 0, 1) - Date.UTC(year, 0, 1)) / DAY)
  const lastMonth = year === new Date(NOW).getUTCFullYear() ? new Date(NOW).getUTCMonth() : 11
  return { total, active: days.length, elapsed, busiest, streak, months, peakMonth, lastMonth }
}

function YearFactsCard({ year, counts, onDay }: { year: number; counts: Map<string, number>; onDay: (key: string) => void }) {
  const f = useMemo(() => yearFacts(year, counts), [year, counts])
  const maxMonth = Math.max(1e-9, ...f.months)
  const h = (v: number) => hours(v * 3_600_000)
  return (
    <SectionCard title={`${year} 年`} className="flex-1">
      {/* Two, three or six to a row, so the six figures never leave one hanging. */}
      <div className="@container">
      <dl className="grid grid-cols-2 gap-x-6 gap-y-4 @min-[400px]:grid-cols-3 @min-[820px]:grid-cols-6">
        <Fact label="Agent 工作" value={h(f.total)} sub="同時開的只算一次" />
        <Fact label="有對話的日子" value={`${num(f.active)} 天`} sub={`${year === new Date(NOW).getUTCFullYear() ? '今年到今天' : '全年'} ${num(f.elapsed)} 天`} />
        <Fact label="每天平均" value={f.active ? h(f.total / f.active) : '—'} sub="只算有對話的日子" />
        <Fact label="最長連續" value={`${num(f.streak.len)} 天`} sub={f.streak.len > 1 ? `${dayText(f.streak.from)} – ${dayText(f.streak.to)}` : undefined} />
        <Fact
          label="最忙的一天"
          value={f.busiest.key ? h(f.busiest.v) : '—'}
          sub={
            f.busiest.key ? (
              <button type="button" onClick={() => onDay(f.busiest.key)} className="hover:text-foreground hover:underline">
                {dayText(f.busiest.key)} →
              </button>
            ) : undefined
          }
        />
        <Fact label="最忙的月份" value={f.total ? `${f.peakMonth + 1} 月` : '—'} sub={f.total ? h(f.months[f.peakMonth]) : undefined} />
      </dl>
      </div>
      {/* Each month's hours, so the year's shape reads at a glance; months still ahead stay empty. */}
      <div className="flex min-h-[48px] flex-1 flex-col gap-1.5" role="img" aria-label={`${year} 年每個月的 Agent 工作時間`}>
        <div className="flex flex-1 items-end gap-1.5 border-b border-foreground/10">
          {f.months.map((v, m) => (
            <span
              key={m}
              title={m > f.lastMonth ? `${m + 1} 月 · 還沒到` : `${m + 1} 月 · ${h(v)}`}
              className={cn('min-h-px flex-1 rounded-t-[3px]', m === f.peakMonth && f.total ? 'bg-heat-4' : 'bg-heat-3', m > f.lastMonth && 'bg-transparent')}
              style={{ height: `${(v / maxMonth) * 100}%` }}
            />
          ))}
        </div>
        <div className="flex gap-1.5 text-[11px] text-muted-foreground tabular">
          {f.months.map((_, m) => (
            <span key={m} className="flex-1 text-center">
              {m + 1}
            </span>
          ))}
        </div>
      </div>
    </SectionCard>
  )
}

function Fact({ label, value, sub }: { label: string; value: ReactNode; sub?: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="text-[17px] font-semibold tabular">{value}</dd>
      {sub && <dd className="truncate text-xs text-muted-foreground tabular">{sub}</dd>}
    </div>
  )
}

// --------------------------------------------------------- projects, tools

export function TopProjectsCard({ ins, scope }: { ins: Insights; scope: Scope }) {
  const top = ins.topProjects.slice(0, 8)
  return (
    <SectionCard
      title="依專案"
      subtitle={`${rangeWindow[scope.range]}，Agent 花最多時間的專案（分開加總）`}
      action={
        <Link to="/agent/projects" className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground">
          <span className="whitespace-nowrap">全部 {num(ins.projectCount)} 個</span> <ArrowRight className="size-3" />
        </Link>
      }
    >
      <ShareList
        items={top.map((p) => {
          const project = archive.projectById.get(p.id)!
          return {
            key: p.id,
            label: (
              <Link to={`/agent/projects/${p.id}`} className="flex min-w-0 items-center gap-1.5 hover:underline">
                <ProjectIcon project={project} />
                <span className="truncate">{project.name}</span>
                <span className="flex shrink-0 gap-0.5">
                  {p.harnesses.map((h) => (
                    <HarnessBadge key={h} id={h} />
                  ))}
                </span>
              </Link>
            ),
            value: p.agentMs,
            display: hours(p.agentMs),
            sub: `${num(p.chats)} 段 · ${big(p.tokens)} token`,
          }
        })}
      />
    </SectionCard>
  )
}

export function ModelsCard({ ins }: { ins: Insights }) {
  const total = ins.models.reduce((s, m) => s + m.tokens, 0)
  const top = ins.models.slice(0, 7)
  return (
    <SectionCard title="模型" subtitle="主模型用掉的 token；subagent 的模型算在它的上層對話裡">
      <ShareList
        items={top.map((m, i) => ({
          key: m.key,
          label: <span className="truncate font-mono text-[12px]">{m.model}</span>,
          value: m.tokens,
          display: m.tokens ? (m.tokens / Math.max(1, total) < 0.01 ? '<1%' : pct(m.tokens / Math.max(1, total))) : '—',
          sub: `${num(m.chats)} 段`,
          color: modelColors[Math.min(i, 5)],
        }))}
      />
    </SectionCard>
  )
}

export function ToolsCard({ ins }: { ins: Insights }) {
  const top = ins.tools.slice(0, 10)
  return (
    <SectionCard title="工具呼叫" subtitle={`共 ${num(ins.totalToolCalls)} 次。名稱照各家原本的叫法，Read 和 read_file 是兩個工具`}>
      <ShareList
        items={top.map((t) => ({
          key: `${t.harness}-${t.tool}`,
          label: (
            <>
              <HarnessBadge id={t.harness} />
              <span className="truncate font-mono text-[12px]">{t.tool}</span>
            </>
          ),
          value: t.count,
          display: big(t.count),
          color: harnessColor[t.harness],
        }))}
      />
    </SectionCard>
  )
}

export function LongestCard({ ins }: { ins: Insights }) {
  return (
    <SectionCard title="最久的對話" subtitle="單一段對話裡 Agent 工作最久的">
      <ul className="-mx-2 flex flex-col">
        {ins.longest.map((chat) => (
          <li key={chat.id}>
            <Link to={`/agent/chats?chat=${chat.id}`} className="flex min-w-0 items-center gap-2.5 rounded-lg px-2 py-2 hover:bg-foreground/[0.035]">
              <MessagesSquare className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-[13px] font-medium">{chat.title}</span>
                <span className="flex items-center gap-1.5 truncate text-xs text-muted-foreground">
                  <HarnessBadge id={chat.harness} />
                  {projectOf(chat).name} · {num(chat.turns)} 輪
                </span>
              </span>
              <span className="shrink-0 text-[13px] font-medium tabular">{duration(chat.agentMs)}</span>
            </Link>
          </li>
        ))}
      </ul>
    </SectionCard>
  )
}

export function CostCard({ ins }: { ins: Insights }) {
  const { estimateCost: estimate, set } = useProto()
  const setEstimate = (v: boolean) => set({ estimateCost: v })
  const cost = costOf(ins.chats)
  return (
    <SectionCard title="費用（按 API 牌價）" subtitle="訂閱方案不按 token 收費；這是換成 API 牌價的等值金額，看用量的分量">
      <div className="flex items-baseline gap-2">
        <span className="text-2xl font-semibold tracking-[-0.02em] tabular">{usd(estimate ? cost.total : cost.recorded)}</span>
        <span className="text-xs text-muted-foreground">{estimate ? '工具記下的＋估算' : '只算工具記下的'}</span>
      </div>
      <dl className="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1.5 text-[13px]">
        <dt>工具自己記下的</dt>
        <dd className="text-right tabular">
          {usd(cost.recorded)} <span className="text-muted-foreground">· {num(cost.recordedChats)} 段</span>
        </dd>
        <dt className={cn(!estimate && 'text-muted-foreground line-through')}>按牌價估算</dt>
        <dd className={cn('text-right tabular', !estimate && 'text-muted-foreground line-through')}>
          {usd(cost.estimated)} <span className="text-muted-foreground">· {num(cost.estimatedChats)} 段</span>
        </dd>
        {cost.unpriced > 0 && (
          <>
            <dt className="text-muted-foreground">沒有牌價（本機模型或沒有用量紀錄）</dt>
            <dd className="text-right text-muted-foreground tabular">{num(cost.unpriced)} 段</dd>
          </>
        )}
      </dl>
      <label className="flex items-center justify-between gap-3 rounded-lg border px-3 py-2 text-[13px]">
        <span className="flex flex-col">
          按公開牌價估算沒有記費用的對話
          <span className="text-xs text-muted-foreground">牌價表 {PRICE_TABLE_VERSION} 版，跟著 PathKeep 更新；在設定 › Agent 對話也能關</span>
        </span>
        <Switch checked={estimate} onCheckedChange={setEstimate} />
      </label>
    </SectionCard>
  )
}

export function MethodNote() {
  return (
    <SectionCard title="這些數字怎麼算">
      <ul className="flex list-disc flex-col gap-1.5 pl-5 text-[13px] leading-relaxed text-muted-foreground">
        <li>
          <span className="text-foreground">一段對話</span>：你開的一次對話。它叫出來的 subagent 算在它裡面，不另算一段。
        </li>
        <li>
          <span className="text-foreground">Agent 工作時間</span>：每一輪從你送出提示到 Agent 做完。Claude Code 從 2025 年 9 月左右開始自己記，其餘從紀錄裡的時間戳推算。「Agent 工作時間」按時鐘算，同時開幾個都只算一次；「分開加總」每段各算各的，兩個一起跑 1 小時就算 2 小時。
        </li>
        <li>
          <span className="text-foreground">Token</span>：工具寫進紀錄的用量。同一則回覆在 Claude Code 的紀錄裡會重複出現好幾次，只算一次；Codex 記的是累計值，取差值；從別的對話分出來的部分不重算。
        </li>
        <li>
          <span className="text-foreground">沒有紀錄的不當成 0</span>：每張卡片都會標出有幾段對話有這項資料。
        </li>
        <li>全部在這台電腦上算，不需要網路，也不需要設定 AI。</li>
      </ul>
    </SectionCard>
  )
}

