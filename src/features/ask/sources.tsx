/**
 * Citation chips under an answer: the history pages and the records of
 * Agent conversations the assistant relied on.
 *
 * Responsibilities: one chip per thing cited, a few at first and the rest
 * on request; a page shows its site and opens History at the visit; a
 * record shows its conversation's tool and title and opens the Agent reader
 * at that record. Not responsible for telling the two apart or building
 * their paths (`citations.ts`).
 */
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import { SiteIcon } from '@/features/history/site-icon'
import { useFormat, useI18n } from '@/lib/i18n'
import { useChatHeader } from '@/lib/queries/agent-chats'
import { spaceById } from '@/lib/spaces'
import type { AiChatCitation, AiCitationTarget } from '@/lib/types'
import { citationKey, citationPath, hostOf, recordOf } from './citations'

const COLLAPSED_COUNT = 6

const chip =
  'flex h-[26px] max-w-[240px] items-center gap-1.5 rounded-full border px-2.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground hover:no-underline'

/** Shown until the conversation's tool is known, or when it can't be read. */
const AgentIcon = spaceById('agent')!.icon

/** The group's name says what it holds: pages, conversations, or both. */
function groupLabel(citations: readonly AiChatCitation[]) {
  const records = citations.filter((citation) => recordOf(citation)).length
  if (records === 0) return 'ask.turn.sources' as const
  return records === citations.length
    ? ('ask.turn.sourcesRecords' as const)
    : ('ask.turn.sourcesMixed' as const)
}

export function Sources({ citations }: { citations: AiChatCitation[] }) {
  const { t } = useI18n()
  const [expanded, setExpanded] = useState(false)
  const shown = expanded ? citations : citations.slice(0, COLLAPSED_COUNT)
  const hidden = citations.length - COLLAPSED_COUNT

  return (
    <div
      role="group"
      aria-label={t(groupLabel(citations))}
      className="flex flex-wrap gap-1.5"
    >
      {shown.map((citation) => {
        const record = recordOf(citation)
        return record ? (
          <RecordChip
            key={citationKey(citation)}
            citation={citation}
            record={record}
          />
        ) : (
          <PageChip key={citationKey(citation)} citation={citation} />
        )
      })}
      {hidden > 0 && (
        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          className="h-[26px] rounded-full px-2.5 text-xs text-muted-foreground hover:text-foreground"
        >
          {expanded
            ? t('ask.turn.lessSources')
            : t('ask.turn.moreSources', { count: hidden })}
        </button>
      )}
    </div>
  )
}

function PageChip({ citation }: { citation: AiChatCitation }) {
  const { t } = useI18n()
  const format = useFormat()
  const host = hostOf(citation.url)
  const title = citation.title?.trim() || host
  return (
    <Link
      to={citationPath(citation)}
      title={
        citation.visitedAt
          ? t('ask.turn.sourceTitle', {
              title,
              date: format.date(citation.visitedAt),
            })
          : title
      }
      className={chip}
    >
      <SiteIcon domain={host} className="size-3.5 rounded-[4px]" />
      <span className="truncate">{title}</span>
    </Link>
  )
}

/**
 * A record of an Agent conversation, under the conversation's own title
 * once its header is read (shared with the reader's cache). Until then, or
 * when it can't be read (the space is off or locked, the conversation is
 * gone), the start of the cited passage stands in.
 */
function RecordChip({
  citation,
  record,
}: {
  citation: AiChatCitation
  record: AiCitationTarget
}) {
  const { t } = useI18n()
  const summary = useChatHeader(record.conversationId).data?.summary
  const passage = citation.title?.trim() ?? ''
  const title =
    summary?.title?.trim() || passage || t('agentChats.row.untitled')
  return (
    <Link
      to={citationPath(citation)}
      title={
        summary && passage
          ? t('ask.turn.recordTitle', {
              title,
              tool: t(toolName(summary.harness)),
              passage,
            })
          : title
      }
      className={chip}
    >
      {summary ? (
        <ToolIcon id={summary.harness} />
      ) : (
        // The tool icon's 16 px box, so the title does not move when it arrives.
        <span
          aria-hidden
          className="inline-flex size-4 shrink-0 items-center justify-center"
        >
          <AgentIcon className="size-3.5" />
        </span>
      )}
      <span className="truncate">{title}</span>
    </Link>
  )
}
