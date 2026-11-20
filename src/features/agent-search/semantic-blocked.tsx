/**
 * What Records says instead of semantic results when it can't search by
 * meaning: a typed refusal (AI, semantic search, the "Build a semantic
 * search index" switch or Agent conversations off; no embedding model, one
 * that changed, a service that didn't answer, an in-app model that isn't
 * ready), the index being built or paused, its last build failed, none
 * built, one the backend won't search with, or a status that couldn't be
 * read.
 *
 * Responsibilities: a title, what to do, a link to each settings page where
 * it is done, Retry where trying again can help, and the way back to
 * full-text search. Not responsible for: deciding which it is
 * (`semantic-gate.ts`, `use-semantic-search.ts`) or the refusals' words
 * (`features/ai-setup/ai-refusals.ts`); nothing here changes a setting.
 */
import {
  CircleAlert,
  CirclePause,
  Hammer,
  SearchSlash,
  Sparkles,
  TriangleAlert,
} from 'lucide-react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty'
import {
  refusalPlaceName,
  refusalPlacePath,
  refusalWords,
  type RefusalPlace,
} from '@/features/ai-setup/ai-refusals'
import { useI18n, type Formatters } from '@/lib/i18n'
import type { SemanticGate } from './semantic-gate'

/** A state that stands in the way, or a search the backend refused as not ready. */
export type SemanticBlock =
  | Exclude<SemanticGate, { kind: 'search' } | { kind: 'loading' }>
  | { kind: 'notReady' }

/** Links to the settings pages where it is done, as Settings › {section}. */
export function PlaceLinks({ places }: { places: RefusalPlace[] }) {
  const { t } = useI18n()
  return places.map((place) => (
    <Button key={place} asChild size="sm">
      <Link to={refusalPlacePath[place]}>
        {t('agentSearch.semantic.settings', {
          section: t(refusalPlaceName[place]),
        })}
      </Link>
    </Button>
  ))
}

export function SemanticBlocked({
  block,
  f,
  onTextInstead,
  onRetry,
}: {
  block: SemanticBlock
  f: Formatters
  onTextInstead: () => void
  /** Read the index's state again and, for a refused search, send it again. */
  onRetry: () => void
}) {
  const { t } = useI18n()
  const passages = (count: number) => ({ count, shown: f.number(count) })
  let icon = <Sparkles />
  let title: string
  let body: string[]
  let places: RefusalPlace[] = ['agent']
  let retry = false
  switch (block.kind) {
    case 'refused': {
      const words = refusalWords(t, block.code, 'semantic')
      title = words.title
      body = [words.body]
      places = words.places
      retry = words.retry
      break
    }
    case 'building':
      if (block.job.state === 'paused') {
        icon = <CirclePause />
        title = t('agentSearch.semantic.blocked.pausedTitle')
        body = [
          t(
            'agentSearch.semantic.blocked.paused',
            passages(block.job.passages),
          ),
        ]
      } else {
        icon = <Hammer />
        title = t('agentSearch.semantic.blocked.buildingTitle')
        body = [
          t(
            'agentSearch.semantic.blocked.building',
            passages(block.job.passages),
          ),
        ]
      }
      break
    case 'failed':
      icon = <TriangleAlert />
      title = t('agentSearch.semantic.blocked.failedTitle')
      body = [t('agentSearch.semantic.blocked.failed')]
      if (block.code) {
        // Why it stopped, and where that is put right, before where to build again.
        const words = refusalWords(t, block.code, 'semantic')
        body.unshift(
          t('agentSearch.semantic.blocked.failedWhy', {
            reason: words.title,
            fix: words.body,
          }),
        )
        places = [...new Set([...words.places, 'agent' as const])]
      }
      break
    case 'notBuilt':
      icon = <SearchSlash />
      title = t('agentSearch.semantic.blocked.notBuiltTitle')
      body = [t('agentSearch.semantic.blocked.notBuilt')]
      break
    case 'notReady':
      icon = <CircleAlert />
      title = t('agentSearch.semantic.blocked.notReadyTitle')
      body = [t('agentSearch.semantic.blocked.notReady')]
      break
    case 'unknown':
      icon = <TriangleAlert />
      title = t('agentSearch.semantic.blocked.unknownTitle')
      body = [block.error.message]
      places = []
      retry = true
      break
  }
  return (
    <Empty
      role={block.kind === 'unknown' ? 'alert' : 'status'}
      data-semantic-block={block.kind === 'refused' ? block.code : block.kind}
      className="animate-rise flex-1 justify-start"
    >
      <EmptyHeader>
        <EmptyMedia variant="icon">{icon}</EmptyMedia>
        <EmptyTitle className="text-base">{title}</EmptyTitle>
        {body.map((line) => (
          <EmptyDescription key={line} className="break-words">
            {line}
          </EmptyDescription>
        ))}
      </EmptyHeader>
      <EmptyContent className="flex-row flex-wrap justify-center gap-2">
        <PlaceLinks places={places} />
        {retry && (
          <Button size="sm" variant="outline" onClick={onRetry}>
            {t('common.retry')}
          </Button>
        )}
        <Button size="sm" variant="outline" onClick={onTextInstead}>
          {t('agentSearch.semantic.textInstead')}
        </Button>
      </EmptyContent>
    </Empty>
  )
}
