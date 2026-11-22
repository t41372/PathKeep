/**
 * Folders that may belong to another project, with the evidence: "把
 * ~/projects/2025/orbit-notes 加到 orbit-notes". The user adds, declines or
 * postpones. Adding only regroups inside PathKeep's archive and can be
 * undone; harness folders are never touched.
 */
import { Check, ChevronDown, Clock, FolderPlus, Info, Undo2, X } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { cn } from '@/lib/cn'
import { archive } from '~/data/archive'
import { num } from '~/data/format'
import type { MergeSuggestion } from '~/data/types'
import { decide, displayName, useProjectEdits, type Decision } from './store'

const kindLabel: Record<MergeSuggestion['kind'], string> = {
  'same-remote': '同一個 git 遠端',
  moved: '資料夾搬過家',
  worktree: '已刪除的 worktree',
  subfolder: '在專案的子資料夾裡',
}

const decisionLabel: Record<Decision, string> = {
  added: '已加入',
  declined: '不是',
  later: '之後再說',
}

export function SuggestionCard({ s, decision }: { s: MergeSuggestion; decision?: Decision }) {
  const edits = useProjectEdits()
  const from = archive.projectById.get(s.fromId)!
  const into = displayName(edits, s.intoId)
  const act = (d: Decision) => {
    decide(s.id, d)
    if (d === 'added') {
      toast(`已把 ${from.locations[0].path} 加到「${into}」`, {
        description: `${num(from.chatIds.length)} 段對話現在算在「${into}」`,
        action: { label: '復原', onClick: () => decide(s.id, null) },
      })
    }
  }
  return (
    <div className={cn('flex flex-col gap-2.5 rounded-xl border bg-card p-4 shadow-card', decision === 'later' && 'opacity-80')}>
      <div className="flex items-center gap-2">
        <Badge variant="secondary">{kindLabel[s.kind]}</Badge>
        {decision === 'later' && (
          <span className="flex items-center gap-1 text-xs text-muted-foreground">
            <Clock className="size-3" />
            你說之後再看
          </span>
        )}
      </div>
      <p className="text-[13.5px] leading-relaxed">
        把 <span className="font-mono text-[12.5px]">{from.locations[0].path}</span> 加到「<span className="font-medium">{into}</span>」？
        <span className="ml-1 text-[12.5px] text-muted-foreground">
          {num(from.chatIds.length)} 段對話{from.gone ? '，資料夾已不在' : ''}
        </span>
      </p>
      <p className="flex gap-1.5 text-[12.5px] leading-relaxed text-muted-foreground">
        <Info className="mt-0.5 size-3.5 shrink-0" />
        {s.evidence}
      </p>
      <div className="flex gap-2">
        <Button size="sm" onClick={() => act('added')}>
          <FolderPlus />
          加進去
        </Button>
        <Button size="sm" variant="ghost" onClick={() => act('declined')}>
          <X />
          不是
        </Button>
        {decision !== 'later' && (
          <Button size="sm" variant="ghost" onClick={() => act('later')}>
            之後再說
          </Button>
        )}
      </div>
    </div>
  )
}

export function SuggestionsSheet({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const edits = useProjectEdits()
  const [showDone, setShowDone] = useState(false)
  const pending = archive.suggestions
    .filter((s) => !edits.decisions.has(s.id) || edits.decisions.get(s.id) === 'later')
    .sort((a, b) => Number(edits.decisions.get(a.id) === 'later') - Number(edits.decisions.get(b.id) === 'later'))
  const done = archive.suggestions.filter((s) => {
    const d = edits.decisions.get(s.id)
    return d === 'added' || d === 'declined'
  })

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="w-[540px] gap-0 sm:max-w-[540px]">
        <SheetHeader className="border-b px-5 pt-5 pb-4">
          <SheetTitle>可能屬於別的專案的資料夾</SheetTitle>
          <SheetDescription className="leading-relaxed">
            一個專案可以包含好幾個資料夾。下面這些資料夾現在各自算一個專案，但看起來屬於別的專案。加進去只改 PathKeep 的分組，不動 Claude Code 或 Codex 的資料夾，之後隨時能在專案設定裡移出。
          </SheetDescription>
        </SheetHeader>
        <div className="flex flex-1 flex-col gap-3 overflow-y-auto px-5 py-4">
          {pending.length === 0 ? (
            <div className="flex flex-col items-center gap-2 py-10 text-center text-[13px] text-muted-foreground">
              <Check className="size-6" />
              都處理完了
            </div>
          ) : (
            pending.map((s) => <SuggestionCard key={s.id} s={s} decision={edits.decisions.get(s.id)} />)
          )}
          {done.length > 0 && (
            <div className="mt-2 flex flex-col gap-1">
              <button type="button" onClick={() => setShowDone((v) => !v)} className="flex items-center gap-1 self-start text-[13px] font-medium text-muted-foreground hover:text-foreground">
                <ChevronDown className={cn('size-4 transition-transform', !showDone && '-rotate-90')} />
                已處理 {done.length}
              </button>
              {showDone &&
                done.map((s) => (
                  <div key={s.id} className="flex items-center gap-2 rounded-lg px-2 py-1.5 text-[13px]">
                    <span className="w-14 shrink-0 text-xs text-muted-foreground">{decisionLabel[edits.decisions.get(s.id)!]}</span>
                    <span className="min-w-0 flex-1 truncate">
                      <span className="font-mono text-[12px]">{archive.projectById.get(s.fromId)!.locations[0].path}</span> → {displayName(edits, s.intoId)}
                    </span>
                    <Button size="xs" variant="ghost" onClick={() => decide(s.id, null)}>
                      <Undo2 />
                      復原
                    </Button>
                  </div>
                ))}
            </div>
          )}
        </div>
      </SheetContent>
    </Sheet>
  )
}
