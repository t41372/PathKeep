/**
 * One conversation in the search results (`agent-history.md` §3.4): the
 * tool, the title and how many matches; where and when; up to three
 * snippets, each with whose words it is and the match lit; how many more
 * there are, or that only the title matched. The title opens the
 * conversation at the first snippet, a snippet at its own record.
 */
import { memo } from 'react'
import {
  OnlyInPathKeepMark,
  PutAwayMark,
} from '@/features/agent-chats/chat-row'
import { rowWhen } from '@/features/agent-chats/chat-words'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import type { SearchCard, SearchMatch } from '@/lib/backend-client/agent-search'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters, type MessageKey } from '@/lib/i18n'
import { utf8RangeToIndexes } from '@/lib/utf8-range'

const roleKey: Record<SearchMatch['role'], MessageKey> = {
  user: 'agentSearch.role.user',
  agent: 'agentSearch.role.agent',
  tool: 'agentSearch.role.tool',
  thinking: 'agentSearch.role.thinking',
  title: 'agentSearch.role.title',
}

/** `text` with the byte range `range` lit. */
export function Lit({
  text,
  range,
}: {
  text: string
  range: readonly [number, number]
}) {
  const [from, to] = utf8RangeToIndexes(text, range)
  return (
    <>
      {text.slice(0, from)}
      <mark className="rounded-[3px] bg-brand-soft px-0.5 text-foreground">
        {text.slice(from, to)}
      </mark>
      {text.slice(to)}
    </>
  )
}

export const ResultCard = memo(function ResultCard({
  card,
  selected,
  f,
  zone,
  onOpen,
}: {
  card: SearchCard
  selected: boolean
  f: Formatters
  zone: string
  /** Opens the conversation at this snippet's record, or at its start. */
  onOpen: (card: SearchCard, snippet: SearchMatch | null) => void
}) {
  const { t } = useI18n()
  const title = card.title?.trim() || t('agentChats.row.untitled')
  const more = Math.max(
    0,
    card.hits - card.snippets.length - (card.titleHit ? 1 : 0),
  )
  return (
    <article
      data-result-id={card.id}
      aria-current={selected ? 'true' : undefined}
      className={cn(
        'flex flex-col gap-1.5 border-b px-4 py-3 transition-colors duration-150',
        selected ? 'bg-accent' : 'hover:bg-muted/60',
      )}
    >
      <div className="flex min-w-0 items-center gap-2">
        <ToolIcon id={card.harness} />
        <span className="sr-only">{t(toolName(card.harness))}</span>
        <button
          type="button"
          onClick={() => onOpen(card, card.snippets[0] ?? null)}
          className="min-w-0 flex-1 truncate text-left text-[13.5px] font-medium hover:underline"
          title={title}
        >
          {title}
        </button>
        <span className="shrink-0 text-[12px] text-muted-foreground tabular">
          {t('agentSearch.card.matches', {
            count: card.hits,
            shown: f.number(card.hits),
          })}
        </span>
      </div>
      <div className="flex min-w-0 items-center gap-2 text-[12px] text-muted-foreground">
        {card.project && <span className="truncate">{card.project.label}</span>}
        <span className="shrink-0 tabular">
          {rowWhen(t, f, zone, card.lastActivityAt, false)}
        </span>
        {card.onlyInPathKeep && <OnlyInPathKeepMark />}
        {card.archived && <PutAwayMark withText />}
      </div>
      {card.snippets.length === 0 ? (
        card.titleHit && (
          <p className="text-[12.5px] text-muted-foreground">
            {t('agentSearch.card.titleOnly')}
          </p>
        )
      ) : (
        <ul className="flex flex-col gap-1">
          {card.snippets.map((snippet, index) => (
            <li key={`${snippet.occurrence}-${index}`}>
              <button
                type="button"
                onClick={() => onOpen(card, snippet)}
                className="flex w-full min-w-0 gap-2 rounded-md px-1.5 py-1 text-left text-[12.5px] leading-snug hover:bg-muted"
              >
                <span className="flex shrink-0 flex-col items-start gap-0.5 pt-px">
                  {snippet.subagentId && (
                    <span className="rounded bg-muted px-1 text-[10.5px] text-muted-foreground">
                      {t('agentSearch.card.subagent')}
                    </span>
                  )}
                  <span className="text-[11.5px] font-medium text-muted-foreground">
                    {t(roleKey[snippet.role])}
                  </span>
                </span>
                <span className="line-clamp-2 min-w-0 [overflow-wrap:anywhere]">
                  <Lit text={snippet.text} range={snippet.highlight} />
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {more > 0 && (
        <p className="text-[12px] text-muted-foreground">
          {t('agentSearch.card.more', { count: more, shown: f.number(more) })}
        </p>
      )}
    </article>
  )
})
