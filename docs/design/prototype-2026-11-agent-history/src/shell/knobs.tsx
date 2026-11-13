/** Prototype-only panel: switch the setup being looked at. Not a product surface. */
import { FlaskConical } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { setTasksRunning } from '~/data/tasks'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { archive } from '~/data/archive'
import { useProto, type KindPreset } from '~/proto-state'

const longest = [...archive.chats].sort((a, b) => b.turns - a.turns)[0]
const goneChat = archive.chats.find((c) => c.source === 'gone' && c.subagentCount > 3)!
const wtChat = archive.chats.find((c) => c.projectId === 'p-orbit' && c.cwd.includes('rekey-progress'))

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      {children}
    </div>
  )
}

export function Knobs() {
  const p = useProto()
  const [tasksOn, setTasksOn] = useState(true)
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="fixed right-4 bottom-4 z-40 flex items-center gap-1.5 rounded-full border border-dashed border-foreground/30 bg-popover/90 px-3 py-1.5 text-xs font-medium text-muted-foreground shadow-float backdrop-blur hover:text-foreground"
        >
          <FlaskConical className="size-3.5" />
          原型選項
        </button>
      </PopoverTrigger>
      <PopoverContent side="top" align="end" className="flex w-[320px] flex-col gap-4">
        <p className="text-xs leading-relaxed text-muted-foreground">只屬於原型的切換，用來看不同設定下的樣子。資料全是假的。</p>
        <Row label="導航">
          <ToggleGroup type="single" variant="outline" size="sm" value={p.nav} onValueChange={(v) => v && p.set({ nav: v as 'spaces' | 'types' })}>
            <ToggleGroupItem value="spaces">切換空間（這一輪）</ToggleGroupItem>
            <ToggleGroupItem value="types">依類型（上一輪）</ToggleGroupItem>
          </ToggleGroup>
        </Row>
        <Row label="開啟的資料類型">
          <ToggleGroup type="single" variant="outline" size="sm" value={p.preset} onValueChange={(v) => v && p.set({ preset: v as KindPreset })}>
            <ToggleGroupItem value="agent">只有 Agent</ToggleGroupItem>
            <ToggleGroupItem value="browsing">只有瀏覽</ToggleGroupItem>
            <ToggleGroupItem value="two">兩種</ToggleGroupItem>
            <ToggleGroupItem value="six">六種</ToggleGroupItem>
          </ToggleGroup>
        </Row>
        {p.nav === 'types' && (
          <label className="flex items-center justify-between gap-3 text-[13px]">
            只開一種時，頁面直接放進導航列
            <Switch checked={p.flatten} onCheckedChange={(v) => p.set({ flatten: v })} />
          </label>
        )}
        <label className="flex items-center justify-between gap-3 text-[13px]">
          升級後還沒開啟 Agent 對話
          <Switch checked={!p.agentEnabled} onCheckedChange={(v) => p.set({ agentEnabled: !v })} />
        </label>
        <label className="flex items-center justify-between gap-3 text-[13px]">
          背景有任務在跑
          <Switch checked={tasksOn} onCheckedChange={(v) => { setTasksOn(v); setTasksRunning(v) }} />
        </label>
        <label className="flex items-center justify-between gap-3 text-[13px]">
          深色
          <Switch checked={p.theme === 'dark'} onCheckedChange={(v) => p.set({ theme: v ? 'dark' : 'light' })} />
        </label>
        <label className="flex items-center justify-between gap-3 text-[13px]">
          玻璃材質（模擬桌面透出來）
          <Switch checked={p.glass} onCheckedChange={(v) => p.set({ glass: v })} />
        </label>
        <Row label="直接跳到">
          <div className="flex flex-col gap-1 text-[13px] [&_a]:text-brand [&_a]:hover:underline">
            <Link to={`/agent/chat/${longest.id}`}>最長的對話（{longest.turns} 輪）</Link>
            <Link to={`/agent/chats?chat=${goneChat.id}`}>只剩 PathKeep 有的對話</Link>
            {wtChat && <Link to={`/agent/chats?chat=${wtChat.id}`}>在已刪除 worktree 裡的對話</Link>}
            <Link to="/agent/projects/p-orbit">點進一個專案（orbit-notes）</Link>
            <Link to="/agent/projects?review=1">可能屬於別的專案的資料夾</Link>
            <Link to="/tasks">背景任務</Link>
            <Link to="/agent/chats?q=cursor">搜尋「cursor」</Link>
            <Link to="/?enable=agent">首次開啟 Agent 對話</Link>
            <Link to="/onboarding">新手引導</Link>
          </div>
        </Row>
      </PopoverContent>
    </Popover>
  )
}
