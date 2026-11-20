/**
 * One passage in Records' semantic results: the tool and the title of the
 * conversation it is in; whose words it is (and that a subagent's), when
 * the conversation was last active, its marks; then the passage itself,
 * up to three lines. Nothing is lit: a passage close in meaning need not
 * share a word with the search. The whole row opens the passage's record
 * in the conversation that holds it (a subagent's opens on its own).
 */
import { memo } from 'react'
import {
  OnlyInPathKeepMark,
  PutAwayMark,
} from '@/features/agent-chats/chat-row'
import { rowWhen } from '@/features/agent-chats/chat-words'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import type {
  SearchRole,
  SemanticPassage,
} from '@/lib/backend-client/agent-search'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters, type MessageKey } from '@/lib/i18n'

const roleKey: Record<SearchRole, MessageKey> = {
  user: 'agentSearch.role.user',
  agent: 'agentSearch.role.agent',
  tool: 'agentSearch.role.tool',
  thinking: 'agentSearch.role.thinking',
}

/**
 * A passage’s identity on the page (`data-passage`), which the E2E spec
 * compares with the backend's stream: `<conversation>/<ordinal>:<source>/<chunk>`.
 */
function passageKey(passage: SemanticPassage) {
  const { conversationId, ordinal, sourceId } = passage.address
  return `${conversationId}/${ordinal}:${sourceId}/${passage.chunk}`
}

export const PassageRow = memo(function PassageRow({
  passage,
  selected,
  f,
  zone,
  onOpen,
}: {
  passage: SemanticPassage
  /** Its record is the one open in the reader. */
  selected: boolean
  f: Formatters
  zone: string
  onOpen: (passage: SemanticPassage) => void
}) {
  const { t } = useI18n()
  const title = passage.title?.trim() || t('agentChats.row.untitled')
  const subagent = passage.address.conversationId !== passage.id
  return (
    <article
      data-passage={passageKey(passage)}
      aria-current={selected ? 'true' : undefined}
      className={cn(
        'relative flex flex-col gap-1 border-b px-4 py-3 transition-colors duration-150',
        selected ? 'bg-accent' : 'hover:bg-muted/60',
      )}
    >
      <div className="flex min-w-0 items-center gap-2">
        <ToolIcon id={passage.harness} />
        <span className="sr-only">{t(toolName(passage.harness))}</span>
        {/* Its ::after covers the row, so the whole row opens it with one tab stop. */}
        <button
          type="button"
          onClick={() => onOpen(passage)}
          title={title}
          className="min-w-0 flex-1 truncate text-left text-[13.5px] font-medium outline-none after:absolute after:inset-0 after:content-[''] hover:underline focus-visible:after:ring-2 focus-visible:after:ring-ring/50 focus-visible:after:ring-inset"
        >
          {title}
        </button>
      </div>
      <div className="flex min-w-0 items-center gap-2 text-[12px] text-muted-foreground">
        <span className="shrink-0 font-medium">{t(roleKey[passage.role])}</span>
        {subagent && (
          <span className="shrink-0 rounded bg-muted px-1 text-[10.5px]">
            {t('agentSearch.card.subagent')}
          </span>
        )}
        <span className="shrink-0 tabular">
          {rowWhen(t, f, zone, passage.lastActivityAt, false)}
        </span>
        {/* Above the row's link, so their tooltips still answer the pointer. */}
        <span className="relative z-10 flex items-center gap-2">
          {passage.onlyInPathKeep && <OnlyInPathKeepMark />}
          {passage.archived && <PutAwayMark withText />}
        </span>
      </div>
      <p className="line-clamp-3 text-[12.5px] leading-snug [overflow-wrap:anywhere]">
        {passage.text}
      </p>
    </article>
  )
})
