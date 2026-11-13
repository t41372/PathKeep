/**
 * Every source PathKeep reads, grouped by data type in registry order. One
 * card, one section per enabled type; a planned type shows a single row
 * saying so. Agent rows say what each backup covers, because "backed up"
 * means more than the chat text (subagents, tool output, deleted chats).
 */
import { Check, EllipsisVertical, FolderOpen, Pause, RefreshCw } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/cn'
import { HarnessBadge } from '~/components/bits'
import { ago, bytes, num, when } from '~/data/format'
import { kindById, kinds, type Kind } from '~/kinds'
import { useProto } from '~/proto-state'
import { browserSources, harnessSources, kindCounts } from './sources-data'

function RowMenu({ name }: { name: string }) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" className="size-8 text-muted-foreground" aria-label={`${name} 的更多動作`}>
          <EllipsisVertical />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem>
          <FolderOpen />
          在 Finder 中顯示
        </DropdownMenuItem>
        <DropdownMenuItem>
          <RefreshCw />
          重新掃描這個來源
        </DropdownMenuItem>
        <DropdownMenuItem>
          <Pause />
          暫停這個來源
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

const covers = ['對話內容', 'subagent', '工具輸出與附件', '已從工具刪除的仍保留'] as const

function AgentRows() {
  return (
    <ul>
      {harnessSources.map((h) => (
        <li key={h.id} className="flex items-center gap-3.5 border-t py-3 pr-3 pl-5">
          <HarnessBadge id={h.id} className="w-7 justify-center" />
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <span className="flex items-center gap-2 text-sm font-medium">
              {h.name}
              {h.autoDelete && (
                <Badge variant="outline" className="border-amber-500/40 text-[11px] font-normal text-amber-700 dark:text-amber-300">
                  會自動刪舊對話
                </Badge>
              )}
            </span>
            <span className="truncate font-mono text-[11.5px] text-muted-foreground">
              {h.root}
              {h.alsoReads && <span className="font-sans"> 和 </span>}
              {h.alsoReads}
            </span>
            <span className="flex flex-wrap gap-x-3 gap-y-0.5 text-[11.5px] text-muted-foreground">
              {covers
                .filter((c) => c !== 'subagent' || h.subagents > 0)
                .map((c) => (
                <span key={c} className="flex items-center gap-1">
                  <Check className="size-3 text-green" strokeWidth={2.5} />
                  {c}
                </span>
              ))}
            </span>
          </div>
          <div className="hidden min-w-28 flex-col items-end text-right sm:flex">
            <span className="font-mono text-[13px] text-muted-foreground tabular">{num(h.chats)} 段</span>
            <span className="text-xs text-muted-foreground">
              {h.subagents > 0 ? `另有 ${num(h.subagents)} 個 subagent` : '沒有 subagent'}
            </span>
          </div>
          <div className="hidden min-w-24 flex-col items-end text-right md:flex">
            <span className="font-mono text-[13px] text-muted-foreground tabular">{bytes(h.sourceBytes)}</span>
            <span className="text-xs text-muted-foreground">{when(h.lastBackup)}</span>
          </div>
          <span className="flex w-16 items-center justify-end gap-1.5 text-xs text-muted-foreground">
            <span className="size-1.5 rounded-full bg-green" />
            正常
          </span>
          <RowMenu name={h.name} />
        </li>
      ))}
    </ul>
  )
}

function BrowserRows() {
  return (
    <ul>
      {browserSources.map((b) => (
        <li key={`${b.name}-${b.profile}`} className="flex items-center gap-3.5 border-t py-3 pr-3 pl-5">
          <img src={b.icon} alt="" className="size-7" />
          <div className="flex min-w-0 flex-1 flex-col">
            <span className="truncate text-sm font-medium">{b.name}</span>
            <span className="truncate text-xs text-muted-foreground">{b.profile}</span>
          </div>
          <div className="hidden min-w-28 flex-col items-end text-right sm:flex">
            <span className="font-mono text-[13px] text-muted-foreground tabular">{num(b.visits)} 筆</span>
            <span className="text-xs text-muted-foreground">最後造訪 {ago(b.lastVisit)}</span>
          </div>
          <div className="hidden min-w-24 flex-col items-end text-right md:flex">
            <span className="text-[13px] text-muted-foreground">已備份</span>
            <span className="text-xs text-muted-foreground">{when(harnessSources[0].lastBackup)}</span>
          </div>
          <span className="flex w-16 items-center justify-end gap-1.5 text-xs text-muted-foreground">
            <span className="size-1.5 rounded-full bg-green" />
            正常
          </span>
          <RowMenu name={`${b.name} ${b.profile}`} />
        </li>
      ))}
    </ul>
  )
}

function KindSection({ kind }: { kind: Kind }) {
  const Icon = kind.icon
  const count = kind.id === 'agent' ? harnessSources.length : kind.id === 'browsing' ? browserSources.length : 0
  return (
    <section className="border-t first:border-t-0">
      <header className="flex items-center gap-2.5 bg-muted/40 px-5 py-2.5">
        <Icon className="size-4 text-muted-foreground" strokeWidth={1.75} />
        <h3 className="text-[13px] font-semibold">{kind.label}</h3>
        <span className="ml-auto text-xs text-muted-foreground">
          {kind.real ? `${count} 個來源 · ${kindCounts[kind.id] ?? ''}` : '規劃中'}
        </span>
      </header>
      {kind.id === 'agent' && <AgentRows />}
      {kind.id === 'browsing' && <BrowserRows />}
      {!kind.real && (
        <p className="border-t px-5 py-3 text-[13px] text-muted-foreground">
          這個類型還沒做，原型裡沒有它的來源。之後會讀：{kind.sources.join('、')}。
        </p>
      )}
    </section>
  )
}

export function SourcesCard() {
  const { enabledKinds } = useProto()
  const shown = kinds.filter((k) => enabledKinds.includes(k.id))
  const hidden = kinds.filter((k) => !enabledKinds.includes(k.id) && k.real)
  return (
    <section className="overflow-hidden rounded-xl border bg-card shadow-card">
      <header className="flex items-center justify-between px-5 py-3.5">
        <h2 className="text-sm font-semibold">來源</h2>
        <Button variant="ghost" size="sm" className="text-muted-foreground">
          <RefreshCw />
          重新掃描
        </Button>
      </header>
      <div className={cn('border-t')}>
        {shown.map((k) => (
          <KindSection key={k.id} kind={kindById[k.id]} />
        ))}
      </div>
      {hidden.length > 0 && (
        <p className="border-t px-5 py-3 text-xs text-muted-foreground">
          沒有開啟：{hidden.map((k) => k.label).join('、')}。以前存下的資料都還在，可以在「設定 › 管理資料類型」重新開啟。
        </p>
      )}
    </section>
  )
}
