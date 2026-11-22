/** Project insights cards: branches and PRs, tools and models. */
import { ChevronDown, GitBranch, GitPullRequest } from 'lucide-react'
import { useState } from 'react'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { HarnessBadge } from '~/components/bits'
import { harnessColor, harnessOrder } from '~/data/archive'
import { num, pct, when } from '~/data/format'
import type { Chat, Project } from '~/data/types'
import { branchesOf, modelsOf } from './derive'

export function BranchesCard({ project, chats }: { project: Project; chats: Chat[] }) {
  const [all, setAll] = useState(false)
  const branches = branchesOf(chats)
  const prs = new Set(chats.flatMap((c) => (c.pr ? [c.pr.number] : [])))
  if (project.kind !== 'repo' || branches.length === 0) {
    return (
      <SectionCard title="分支與 PR">
        <p className="text-[13px] text-muted-foreground">
          {project.kind === 'scratch' ? '暫存資料夾沒有分支資訊。' : '這個資料夾不是 git repo，沒有分支資訊。'}
        </p>
      </SectionCard>
    )
  }
  const shown = all ? branches : branches.slice(0, 8)
  return (
    <SectionCard title="分支與 PR" subtitle={`${num(branches.length)} 個分支、${num(prs.size)} 個 PR 有對話`}>
      <div className="-mx-2 flex flex-col">
        {shown.map((b) => (
          <div key={b.branch} className="flex items-center gap-3 rounded-lg px-2 py-1.5 text-[13px]">
            <GitBranch className="size-3.5 shrink-0 text-muted-foreground" />
            <span className="min-w-0 truncate font-mono text-[12.5px]">{b.branch}</span>
            <span className="flex shrink-0 gap-1">
              {b.harnesses.map((h) => (
                <HarnessBadge key={h} id={h} />
              ))}
            </span>
            <span className="ml-auto flex shrink-0 items-center gap-1.5">
              {b.prs.slice(0, 3).map((n) => (
                <span key={n} className="flex items-center gap-0.5 rounded-md bg-muted px-1.5 py-px font-mono text-[11px] text-muted-foreground">
                  <GitPullRequest className="size-3" />#{n}
                </span>
              ))}
            </span>
            <span className="w-[52px] shrink-0 text-right text-xs text-muted-foreground tabular">{num(b.chats)} 段</span>
            <span className="w-[76px] shrink-0 text-right text-xs text-muted-foreground">{when(b.lastActive).replace(/ \d\d:\d\d$/, '')}</span>
          </div>
        ))}
      </div>
      {branches.length > 8 && (
        <Button variant="ghost" size="sm" className="self-start text-xs" onClick={() => setAll((v) => !v)}>
          <ChevronDown className={all ? 'rotate-180' : undefined} />
          {all ? '收起' : `還有 ${branches.length - 8} 個分支`}
        </Button>
      )}
    </SectionCard>
  )
}

export function ModelsCard({ chats }: { chats: Chat[] }) {
  const models = modelsOf(chats)
  const used = harnessOrder.filter((h) => chats.some((c) => c.harness === h))
  return (
    <SectionCard title="工具與模型" subtitle={`${num(chats.length)} 段對話`} className="self-start">
      <div className="flex flex-col gap-1.5">
        <div className="flex h-1.5 overflow-hidden rounded-full bg-muted">
          {used.map((h) => (
            <span key={h} style={{ width: pct(chats.filter((c) => c.harness === h).length / chats.length, 2), background: harnessColor[h] }} />
          ))}
        </div>
        {used.map((h) => {
          const n = chats.filter((c) => c.harness === h).length
          return (
            <span key={h} className="flex items-center gap-1.5 text-[12.5px]">
              <span className="size-2 rounded-[2px]" style={{ background: harnessColor[h] }} />
              <HarnessBadge id={h} withName className="text-[12.5px]" />
              <span className="ml-auto text-muted-foreground tabular">
                {num(n)} 段 · {pct(n / chats.length)}
              </span>
            </span>
          )
        })}
      </div>
      <div className="flex flex-wrap gap-1">
        {models.slice(0, 8).map(([m, n]) => (
          <span key={m} className="rounded-md bg-muted px-1.5 py-px font-mono text-[11px]" title={`${n} 段對話`}>
            {m}
          </span>
        ))}
        {models.length > 8 && <span className="text-xs text-muted-foreground">還有 {models.length - 8} 個</span>}
      </div>
    </SectionCard>
  )
}
