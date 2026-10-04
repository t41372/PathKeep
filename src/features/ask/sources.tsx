/** Citation chips under an answer: the history pages the assistant relied on. */
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { Favicon } from '@/components/app/favicon'
import { useFormat, useI18n } from '@/lib/i18n'
import { historyLink, hostOf } from './citations'
import type { AiChatCitation } from '@/lib/types'

const COLLAPSED_COUNT = 6

export function Sources({ citations }: { citations: AiChatCitation[] }) {
  const { t } = useI18n()
  const format = useFormat()
  const [expanded, setExpanded] = useState(false)
  const shown = expanded ? citations : citations.slice(0, COLLAPSED_COUNT)
  const hidden = citations.length - COLLAPSED_COUNT

  return (
    <div
      role="group"
      aria-label={t('ask.turn.sources')}
      className="flex flex-wrap gap-1.5"
    >
      {shown.map((citation) => {
        const host = hostOf(citation.url)
        const title = citation.title?.trim() || host
        return (
          <Link
            key={citation.canonicalUrl || citation.url}
            to={historyLink(citation)}
            title={
              citation.visitedAt
                ? t('ask.turn.sourceTitle', {
                    title,
                    date: format.date(citation.visitedAt),
                  })
                : title
            }
            className="flex h-[26px] max-w-[240px] items-center gap-1.5 rounded-full border px-2.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground hover:no-underline"
          >
            <Favicon domain={host} className="size-3.5 rounded-[4px]" />
            <span className="truncate">{title}</span>
          </Link>
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
