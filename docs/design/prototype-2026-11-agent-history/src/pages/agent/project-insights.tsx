/**
 * A project's insights, opened from the 洞察 button in the project's
 * header: how long agents worked on it, how many tokens, how much changed,
 * week by week, on which branches, with which tools and models. Managing
 * the project (name, folders) lives in its menu, not here.
 */
import { ChevronLeft } from 'lucide-react'
import { useMemo } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { PageHeader } from '@/components/app/section-card'
import { FluidPage } from '~/components/fluid-page'
import { Button } from '@/components/ui/button'
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from '@/components/ui/empty'
import { Coverage, ProjectIcon, Stat } from '~/components/bits'
import { agentTime, archive } from '~/data/archive'
import { totalTokens } from '~/data/fixture'
import { big, hours, num, pct } from '~/data/format'
import type { Chat } from '~/data/types'
import { ActivityCard } from './projects/activity-card'
import { chatsOf } from './projects/derive'
import { BranchesCard, ModelsCard } from './projects/side-cards'
import { displayName, joinedInto, useProjectEdits } from './projects/store'
import { timeWords } from '~/data/labels'

export default function ProjectInsightsPage() {
  const { projectId = '' } = useParams()
  const edits = useProjectEdits()
  const navigate = useNavigate()
  const project = archive.projectById.get(projectId)
  const joined = joinedInto(edits)
  const absorbed = useMemo(
    () => [...joined.entries()].filter(([, into]) => into === projectId).map(([from]) => archive.projectById.get(from)!),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [edits, projectId],
  )
  const chats = useMemo<Chat[]>(() => (project ? [project, ...absorbed].flatMap(chatsOf).sort((a, b) => a.id - b.id) : []), [project, absorbed])
  const time = useMemo(() => agentTime(chats), [chats])

  if (!project || chats.length === 0) {
    return (
      <FluidPage>
        <Empty className="mt-16">
          <EmptyHeader>
            <EmptyTitle>找不到這個專案</EmptyTitle>
            <EmptyDescription>它的資料夾可能已經加到別的專案裡了。</EmptyDescription>
          </EmptyHeader>
          <Button variant="outline" size="sm" onClick={() => void navigate('/agent/projects')}>
            回到專案
          </Button>
        </Empty>
      </FluidPage>
    )
  }

  const name = displayName(edits, project.id)
  const known = chats.filter((c) => c.tokens)
  const tokens = known.reduce((s, c) => s + totalTokens(c.tokens!), 0)
  const cacheRead = known.reduce((s, c) => s + c.tokens!.cacheRead, 0)
  const subagents = chats.reduce((s, c) => s + c.subagentCount, 0)
  const added = chats.reduce((s, c) => s + c.linesAdded, 0)
  const removed = chats.reduce((s, c) => s + c.linesRemoved, 0)
  const files = chats.reduce((s, c) => s + c.filesTouched, 0)
  const recorded = chats.filter((c) => c.agentMsSource === 'recorded').length

  return (
    <FluidPage>
      <div className="-mb-3">
        <Link to={`/agent/projects/${project.id}`} className="inline-flex items-center gap-1 text-[13px] text-muted-foreground hover:text-foreground">
          <ChevronLeft className="size-3.5" />
          {name} 的對話
        </Link>
      </div>
      <PageHeader
        title={
          <span className="flex items-center gap-2.5">
            <ProjectIcon project={project} className="size-6" />「{name}」的洞察
          </span>
        }
        subtitle={absorbed.length > 0 ? `含你加進來的 ${absorbed.length} 個資料夾` : undefined}
      />
      <div className="grid grid-cols-[repeat(auto-fit,minmax(200px,1fr))] gap-3">
        <Stat
          label={timeWords.agent}
          value={hours(time.busy)}
          sub={
            <span className="flex flex-col gap-0.5">
              <span>
                {timeWords.summed} {hours(time.summed)}
                {time.concurrency >= 1.05 && ` · ${timeWords.concurrency(time.concurrency)}`}
              </span>
              <span className="text-[11.5px] text-muted-foreground/80">{timeWords.agentHow}</span>
            </span>
          }
          hint={
            <>
              {timeWords.summedHow}
              {recorded < chats.length && ` ${num(recorded)} / ${num(chats.length)} 段對話的時長是工具記錄的，其餘從時間戳推算。`}
            </>
          }
        />
        <Stat
          label="Token"
          value={known.length ? big(tokens) : '—'}
          sub={known.length < chats.length ? <Coverage known={known.length} total={chats.length} what="用量" /> : `其中 ${pct(cacheRead / tokens)} 是讀取快取`}
        />
        <Stat label="對話" value={num(chats.length)} sub={subagents ? `另有 ${num(subagents)} 個 subagent` : undefined} />
        <Stat
          label="改動行數"
          value={
            <span>
              <span className="text-green">+{big(added)}</span> <span className="text-[18px] text-red">−{big(removed)}</span>
            </span>
          }
          sub={`動過 ${num(files)} 個檔案`}
          hint="從 Agent 的編輯紀錄（Edit、apply_patch 的 diff）加總，各對話分開算，同一行被改兩次會算兩次。"
        />
      </div>
      <ActivityCard chats={chats} />
      <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[1000px]:grid-cols-1">
        <BranchesCard project={project} chats={chats} />
        <ModelsCard chats={chats} />
      </div>
    </FluidPage>
  )
}
