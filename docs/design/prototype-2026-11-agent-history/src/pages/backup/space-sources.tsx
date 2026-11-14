/**
 * One space's sources for its own Backup page: a row per source with what
 * you check first (how much, when, is it fine) and its one control, the
 * switch. Expanding a row shows the folders read and what a backup of it
 * covers. Everything that decides what a backup reads is on this card, in
 * both spaces: the switches, adding a folder, and reading `.git`.
 */
import { Check, ChevronDown, EllipsisVertical, FolderGit2, FolderOpen, FolderPlus, RefreshCw } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/cn'
import { HarnessIcon } from '~/components/bits'
import { ago, bytes, num, when } from '~/data/format'
import type { KindId } from '~/kinds'
import { MovedFrom } from '../settings/parts'
import { browserSources, harnessSources } from './sources-data'

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
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function SourceRow({ icon, name, badge, count, last, detail }: { icon: ReactNode; name: string; badge?: ReactNode; count: string; last: string; detail: ReactNode }) {
  const [open, setOpen] = useState(false)
  const [on, setOn] = useState(true)
  return (
    <Collapsible open={open} onOpenChange={setOpen} asChild>
      <li className="border-t first:border-t-0">
        <div className="flex items-center gap-3 py-2.5 pr-3 pl-4">
          <Switch checked={on} onCheckedChange={setOn} aria-label={`備份 ${name}`} />
          <CollapsibleTrigger className="group flex min-w-0 flex-1 items-center gap-3 text-left" aria-label={`${name}：看保存了哪些部分`}>
            <span className={cn('flex size-7 shrink-0 items-center justify-center transition-opacity', !on && 'opacity-50')}>{icon}</span>
            <span className={cn('flex min-w-0 flex-1 items-center gap-2 text-sm font-medium transition-opacity', !on && 'opacity-60')}>
              <span className="truncate">{name}</span>
              {badge}
            </span>
            <span className="hidden w-28 shrink-0 text-right text-[13px] text-muted-foreground tabular sm:block">{count}</span>
            <span className="hidden w-24 shrink-0 text-right text-[13px] text-muted-foreground md:block">{last}</span>
            <span className="flex w-[72px] shrink-0 items-center justify-end gap-1.5 text-xs text-muted-foreground">
              <span className={cn('size-1.5 rounded-full', on ? 'bg-green' : 'bg-muted-foreground/50')} />
              {on ? '正常' : '不備份'}
            </span>
            <ChevronDown className="size-4 shrink-0 text-muted-foreground transition-transform group-data-[state=open]:rotate-180" />
          </CollapsibleTrigger>
          <RowMenu name={name} />
        </div>
        <CollapsibleContent>
          <div className="flex flex-col gap-2 pr-5 pb-3.5 pl-[108px] text-[12.5px] text-muted-foreground">
            {!on && <span>關著：不備份它。已經存下的留在存檔裡，照樣能看、能搜。</span>}
            {detail}
          </div>
        </CollapsibleContent>
      </li>
    </Collapsible>
  )
}

const covers = ['對話內容', 'subagent', '工具輸出與附件', '已從工具刪除的仍保留'] as const

function Covered({ items }: { items: readonly string[] }) {
  return (
    <span className="flex flex-wrap gap-x-3 gap-y-0.5">
      {items.map((c) => (
        <span key={c} className="flex items-center gap-1">
          <Check className="size-3 text-green" strokeWidth={2.5} />
          {c}
        </span>
      ))}
    </span>
  )
}

export function SpaceSources({ space }: { space: KindId }) {
  const [git, setGit] = useState(true)
  return (
    <section className="overflow-hidden rounded-xl border bg-card shadow-card">
      <header className="flex items-center gap-3 px-5 py-3">
        <h2 className="text-sm font-semibold">來源</h2>
        <span className="text-xs text-muted-foreground">開關決定備不備份它；點一列看保存了哪些部分</span>
        {space === 'agent' && <MovedFrom label="上一版原型在">設定 › Agent 對話</MovedFrom>}
        <span className="flex-1" />
        {space === 'agent' ? (
          <Button variant="outline" size="sm">
            <FolderPlus />
            加入其他資料夾…
          </Button>
        ) : (
          <Button variant="outline" size="sm">
            <RefreshCw />
            重新掃描
          </Button>
        )}
      </header>
      <ul className="border-t">
        {space === 'agent' &&
          harnessSources.map((h) => (
            <SourceRow
              key={h.id}
              icon={<HarnessIcon id={h.id} className="size-5 [&_img]:size-5" />}
              name={h.name}
              badge={
                h.autoDelete && (
                  <Badge variant="outline" className="border-amber-500/40 text-[11px] font-normal text-amber-700 dark:text-amber-300">
                    會自動刪舊對話
                  </Badge>
                )
              }
              count={`${num(h.chats)} 段`}
              last={when(h.lastBackup)}
              detail={
                <>
                  <span className="font-mono text-[11.5px]">
                    {h.root}
                    {h.alsoReads && <span className="font-sans"> 和 </span>}
                    {h.alsoReads}
                  </span>
                  <Covered items={covers.filter((c) => c !== 'subagent' || h.subagents > 0)} />
                  <span>
                    {h.subagents > 0 ? `另有 ${num(h.subagents)} 個 subagent · ` : ''}來源 {bytes(h.sourceBytes)} · 只剩 PathKeep 有 {num(h.gone)} 段
                  </span>
                </>
              }
            />
          ))}
        {space === 'browsing' &&
          browserSources.map((b) => (
            <SourceRow
              key={`${b.name}-${b.profile}`}
              icon={<img src={b.icon} alt="" className="size-6" />}
              name={`${b.name} · ${b.profile}`}
              count={`${num(b.visits)} 筆`}
              last={when(harnessSources[0].lastBackup)}
              detail={
                <>
                  <span>最後一次造訪是 {ago(b.lastVisit)}</span>
                  <Covered items={['造訪紀錄', '網址與標題', '下載與搜尋字詞', '瀏覽器自己清掉的仍保留']} />
                </>
              }
            />
          ))}
      </ul>
      {space === 'agent' && (
        <div className="flex items-start gap-3 border-t bg-muted/30 px-5 py-3.5">
          <FolderGit2 className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="text-sm font-medium">
              備份時也讀專案資料夾的 .git
              <MovedFrom label="上一版原型在">設定 › Agent 對話</MovedFrom>
            </span>
            <span className="text-[12.5px] leading-relaxed text-muted-foreground">
              只看 repo 的根目錄、worktree 屬於哪個 repo、目前分支、遠端網址，用來把同一個專案的資料夾併在一起。資料夾刪掉之後就查不到了，所以要在備份當下記下來。不讀程式碼，也不改任何檔案。
            </span>
          </span>
          <Switch checked={git} onCheckedChange={setGit} aria-label="備份時也讀專案資料夾的 .git" />
        </div>
      )}
    </section>
  )
}
