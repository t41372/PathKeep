/**
 * The line under History's results that says what semantic results come
 * from, or, when semantic search was asked for but cannot run, why these are
 * full-text results instead. A search that ran can still have fallen back or
 * used another index (the response's notes: an empty index, a previous
 * model, an unreachable service…); each note gets its sentence. The link
 * goes to Settings → AI; nothing here turns anything on.
 */
import { Info, Sparkles } from 'lucide-react'
import { Link } from 'react-router-dom'
import { useFormat, useI18n } from '@/lib/i18n'
import type { AiSearchNote } from '@/lib/types'
import type { SemanticInfo } from './semantic-info'

const settingsLink =
  'shrink-0 rounded text-foreground underline-offset-2 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring/50'

function noteText(
  note: AiSearchNote,
  t: ReturnType<typeof useI18n>['t'],
): string {
  switch (note.code) {
    case 'stale':
      return t(`historySearch.semantic.stale.${note.reason}`)
    // A failed connection's reason is the service's own English; it stays in the note for
    // Ask's trace, and the link below leads to where it can be fixed.
    default:
      return t(`historySearch.semantic.note.${note.code}`)
  }
}

export function SemanticNote({
  info,
  active,
  notes,
}: {
  info: SemanticInfo
  /** Semantic results are on screen (otherwise: asked for, but fell back). */
  active: boolean
  /** What the search that produced these results reported. */
  notes: AiSearchNote[]
}) {
  const { t } = useI18n()
  const format = useFormat()

  if (!active) {
    return (
      <p
        role="status"
        className="flex flex-wrap items-center gap-x-1.5 px-7 pb-1.5 text-xs text-muted-foreground"
      >
        <Info className="size-3.5 shrink-0" aria-hidden />
        <span>
          {t(`historySearch.semantic.reason.${info.reason ?? 'off'}`)}{' '}
          {t('historySearch.semantic.fellBack')}
        </span>
        <Link to="/settings/ai" className={settingsLink}>
          {t('historySearch.semantic.openSettings')}
        </Link>
      </p>
    )
  }

  // While the chosen model's index is unfinished, the previous model answers.
  const previous = notes.find((note) => note.code === 'previousModel')
  const provider = previous ? previous.model : info.provider
  // Staleness comes with the status too, and is said once, below.
  const said = notes.filter((note) => !(note.code === 'stale' && info.stale))
  const parts = [
    provider && t('historySearch.semantic.using', { provider }),
    t('historySearch.semantic.indexed', { count: info.indexed }),
    info.lastIndexedAt &&
      t('historySearch.semantic.updated', {
        when: format.relative(info.lastIndexedAt),
      }),
  ].filter(Boolean)

  return (
    <p className="flex flex-wrap items-center gap-x-1.5 px-7 pb-1.5 text-xs text-muted-foreground">
      <Sparkles className="size-3.5 shrink-0 text-brand" aria-hidden />
      <span>{parts.join(' · ')}</span>
      {info.stale && (
        <>
          <span>· {t(`historySearch.semantic.stale.${info.stale}`)}</span>
          <Link to="/settings/ai" className={settingsLink}>
            {t('historySearch.semantic.rebuild')}
          </Link>
        </>
      )}
      {said.length > 0 && (
        <span role="status" className="flex basis-full flex-wrap gap-x-1.5">
          <span>{said.map((note) => noteText(note, t)).join(' ')}</span>
          <Link to="/settings/ai" className={settingsLink}>
            {t('historySearch.semantic.openSettings')}
          </Link>
        </span>
      )}
    </p>
  )
}
