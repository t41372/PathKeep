/** A project's last 26 weeks, stacked by harness: chats, agent hours or tokens. */
import { useMemo, useState } from 'react'
import { SectionCard } from '@/components/app/section-card'
import { EvilBarChart } from '@/components/evilcharts/charts/recharts-bar-chart'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { harnessColor, harnesses, harnessOrder } from '~/data/archive'
import { timeWords } from '~/data/labels'
import type { Chat } from '~/data/types'
import { weeklyByHarness, type Metric } from './derive'

export { harnessColor }

const subtitle: Record<Metric, string> = {
  chats: '最近 26 週，每週開始的對話數，依工具分開',
  agent: '最近 26 週，每週 Agent 工作的小時數（分開加總），依工具分開',
  tokens: '最近 26 週，每週用掉的 token（單位：萬），沒有用量紀錄的對話不算在內',
}

export function ActivityCard({ chats }: { chats: Chat[] }) {
  const [metric, setMetric] = useState<Metric>('chats')
  const data = useMemo(() => weeklyByHarness(chats, metric), [chats, metric])
  const used = harnessOrder.filter((h) => chats.some((c) => c.harness === h))
  const config = useMemo(
    () =>
      Object.fromEntries(
        used.map((h) => [h, { label: harnesses[h].name, colors: { light: [harnessColor[h]], dark: [harnessColor[h]] } }]),
      ),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [used.join()],
  )
  const empty = data.every((row) => used.every((h) => !row[h]))

  return (
    <SectionCard
      title="每週活動"
      subtitle={subtitle[metric]}
      action={
        <ToggleGroup type="single" variant="outline" size="sm" value={metric} onValueChange={(v) => v && setMetric(v as Metric)}>
          <ToggleGroupItem value="chats" className="text-xs">對話數</ToggleGroupItem>
          <ToggleGroupItem value="agent" className="text-xs" title={timeWords.summedHow}>{timeWords.summed}</ToggleGroupItem>
          <ToggleGroupItem value="tokens" className="text-xs">Token</ToggleGroupItem>
        </ToggleGroup>
      }
    >
      <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
        {used.map((h) => (
          <span key={h} className="flex items-center gap-1.5">
            <span className="size-2 rounded-[2px]" style={{ background: harnessColor[h] }} />
            {harnesses[h].name}
          </span>
        ))}
      </div>
      <div className="relative h-[200px]">
        <EvilBarChart data={data} config={config} xDataKey="week" stackType="stacked" className="aspect-auto h-full" barRadius={2}>
          <EvilBarChart.Grid vertical={false} strokeDasharray="3 4" />
          <EvilBarChart.XAxis dataKey="week" tickLine={false} axisLine={false} minTickGap={28} className="font-mono" />
          <EvilBarChart.Tooltip />
          {used.map((h) => (
            <EvilBarChart.Bar key={h} dataKey={h} />
          ))}
        </EvilBarChart>
        {empty && (
          <div className="absolute inset-0 flex items-center justify-center text-[13px] text-muted-foreground">
            最近 26 週沒有對話
          </div>
        )}
      </div>
    </SectionCard>
  )
}
