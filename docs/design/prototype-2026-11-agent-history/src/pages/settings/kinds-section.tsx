/**
 * Managing data types. Each type is 開啟 / 暫停 / 關閉, and none of the three
 * deletes anything: paused stops backing up but stays in the rail, off also
 * leaves the rail, and the archive keeps everything either way. Deleting a
 * type's archive is a separate, confirmed action. Turning Agent on goes
 * through the preview on Home first.
 */
import { Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { SettingsSection } from '@/components/app/setting-row'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { cn } from '@/lib/cn'
import { totals } from '~/data/archive'
import { bytes, num } from '~/data/format'
import { kinds, type Kind, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'
import { archiveBytes, kindCounts } from '../backup/sources-data'

type State = 'on' | 'paused' | 'off'

const stateLabel: Record<State, string> = {
  on: '開啟',
  paused: '暫停',
  off: '關閉',
}

const stateHint: Record<State, string> = {
  on: '定時備份，導航列裡看得到',
  paused: '不再備份，已存的照樣能看、能搜',
  off: '不再備份，也不在導航列顯示；已存的資料保留',
}

function DeleteData({ kind }: { kind: Kind }) {
  const size = archiveBytes[kind.id]
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="ghost" size="sm" className="-ml-2 w-fit text-destructive hover:text-destructive">
          <Trash2 />
          刪除「{kind.label}」的存檔…
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>刪除存檔裡所有的「{kind.label}」？</AlertDialogTitle>
          <AlertDialogDescription className="leading-relaxed">
            會從 PathKeep 的存檔刪掉 {kindCounts[kind.id]}（{size ? bytes(size) : '—'}），無法復原。
            {kind.id === 'agent' && ` 其中 ${num(totals.gone)} 段在工具自己的資料夾裡已經沒有了，刪掉就再也找不回來。`}
            工具與瀏覽器自己的資料夾不受影響。
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>取消</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => toast('原型：不會真的刪除')}>
            刪除
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

function KindCard({ kind, state, onChange }: { kind: Kind; state: State | 'planned'; onChange: (next: State) => void }) {
  const Icon = kind.icon
  return (
    <div className={cn('flex flex-col gap-3 rounded-xl border bg-card px-[18px] py-4 shadow-card', state === 'planned' && 'bg-card/50 shadow-none')}>
      <div className="flex items-start gap-3.5">
        <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-muted">
          <Icon className="size-[18px]" strokeWidth={1.75} />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="flex items-center gap-2 font-medium">
            {kind.label}
            {state === 'planned' && (
              <Badge variant="outline" className="font-normal text-muted-foreground">
                規劃中
              </Badge>
            )}
          </span>
          <span className="text-[13px] leading-[1.45] text-muted-foreground">{kind.why}</span>
          <span className="text-xs text-muted-foreground">來源：{kind.sources.join('、')}</span>
        </div>
        {state !== 'planned' && (
          <ToggleGroup
            type="single"
            variant="outline"
            size="sm"
            value={state}
            onValueChange={(v) => v && v !== state && onChange(v as State)}
            aria-label={`${kind.label}的狀態`}
          >
            {(['on', 'paused', 'off'] as const).map((s) => (
              <ToggleGroupItem key={s} value={s} className="px-3">
                {stateLabel[s]}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        )}
      </div>
      {state === 'planned' ? (
        <p className="pl-[50px] text-xs text-muted-foreground">還沒做。原型裡用它來看導航列和設定在類型變多時放不放得下。</p>
      ) : (
        <div className="flex flex-col gap-2 border-t pt-3 pl-[50px]">
          <span className="text-xs text-muted-foreground">
            {stateHint[state]}。存檔裡有 {kindCounts[kind.id]}，{archiveBytes[kind.id] ? bytes(archiveBytes[kind.id]!) : '—'}。
          </span>
          <DeleteData kind={kind} />
        </div>
      )}
    </div>
  )
}

export function KindsSection() {
  const { enabledKinds, preset, set } = useProto()
  const navigate = useNavigate()
  const [paused, setPaused] = useState<Set<KindId>>(new Set())

  const stateOf = (kind: Kind): State | 'planned' => {
    if (!kind.real) return 'planned'
    if (!enabledKinds.includes(kind.id)) return 'off'
    return paused.has(kind.id) ? 'paused' : 'on'
  }

  /** Map the two real types onto the prototype's presets. */
  const show = (browsing: boolean, agent: boolean) => {
    const keepSix = preset === 'six'
    if (browsing && agent) set({ preset: keepSix ? 'six' : 'two', agentEnabled: true })
    else if (browsing) set({ preset: keepSix ? 'six' : preset === 'browsing' ? 'browsing' : 'two', agentEnabled: false })
    else set({ preset: 'agent', agentEnabled: true })
  }

  const change = (kind: Kind, next: State) => {
    const browsingOn = enabledKinds.includes('browsing')
    const agentOn = enabledKinds.includes('agent')
    const visible = { browsing: browsingOn, agent: agentOn }
    if (next === 'off') {
      const others = kinds.filter((k) => k.real && k.id !== kind.id && enabledKinds.includes(k.id))
      if (others.length === 0) {
        toast('至少要開著一種資料類型')
        return
      }
      setPaused((p) => new Set([...p].filter((id) => id !== kind.id)))
      show(kind.id === 'browsing' ? false : visible.browsing, kind.id === 'agent' ? false : visible.agent)
      toast(`${kind.label}已關閉，已存的資料都還在`)
      return
    }
    setPaused((p) => {
      const n = new Set(p)
      if (next === 'paused') n.add(kind.id)
      else n.delete(kind.id)
      return n
    })
    const wasOff = !enabledKinds.includes(kind.id)
    if (wasOff && kind.id === 'agent' && next === 'on') {
      // First time on: preview what will be read before anything is backed up.
      // The type turns on when the first backup finishes (or goes to the background).
      void navigate({ search: '?enable=agent' })
      return
    }
    if (wasOff) show(kind.id === 'browsing' ? true : visible.browsing, kind.id === 'agent' ? true : visible.agent)
  }

  return (
    <SettingsSection title="管理資料類型">
      <p className="-mt-2 mb-1 text-[13px] leading-relaxed text-muted-foreground">
        每一種資料各自開關。暫停和關閉都不會刪任何東西：已存的留在存檔裡，重新開啟就回來。要刪掉已存的資料，是另外的動作。
      </p>
      {kinds.map((k) => (
        <KindCard key={k.id} kind={k} state={stateOf(k)} onChange={(next) => change(k, next)} />
      ))}
    </SettingsSection>
  )
}
