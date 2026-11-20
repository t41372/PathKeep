/**
 * What Records says instead of semantic results when it can't search by
 * meaning: AI settings off, the "Build a semantic search index" switch
 * off, no embedding model, the index being built or paused, its last
 * build failed, none built, or one the backend won't search with.
 *
 * Responsibilities: one plain sentence of what to do, a link to the
 * settings page where it is done, and the way back to full-text search.
 * Not responsible for: deciding which it is (`semantic-gate.ts`); nothing
 * here changes a setting.
 */
import {
  CircleAlert,
  Hammer,
  CirclePause,
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
import { useI18n, type Formatters, type MessageKey } from '@/lib/i18n'
import type { SemanticGate } from './semantic-gate'

const AI_SETTINGS = '/settings/ai'
const AGENT_SETTINGS = '/settings/agent'

/** A state that stands in the way, or a search the backend refused as not ready. */
export type SemanticBlock =
  | Exclude<SemanticGate, { kind: 'ready' } | { kind: 'loading' }>
  | { kind: 'notReady' }

type Place = 'ai' | 'agent'

const sectionName: Record<Place, MessageKey> = {
  ai: 'settings.nav.ai',
  agent: 'settings.nav.agent',
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
  /** Read the index's state again (it could not be read). */
  onRetry: () => void
}) {
  const { t } = useI18n()
  const names = {
    ai: t('settingsAi.semantic.title'),
    switch: t('settingsAgent.ai.semantic'),
  }
  const passages = (count: number) => ({ count, shown: f.number(count) })
  let icon = <Sparkles />
  let title: string
  let body: string[]
  let places: Place[]
  switch (block.kind) {
    case 'aiOff':
      title = t('agentSearch.semantic.blocked.aiOffTitle')
      body = [t('agentSearch.semantic.blocked.aiOff', names)]
      places = ['ai']
      if (block.switchOff) {
        body.push(t('agentSearch.semantic.blocked.aiOffSwitch', names))
        places.push('agent')
      }
      break
    case 'switchOff':
      title = t('agentSearch.semantic.blocked.switchOffTitle')
      body = [t('agentSearch.semantic.blocked.switchOff', names)]
      places = ['agent']
      break
    case 'off':
      title = t('agentSearch.semantic.blocked.offTitle')
      body = [t('agentSearch.semantic.blocked.off', names)]
      places = ['ai', 'agent']
      break
    case 'noModel':
      title = t('agentSearch.semantic.blocked.noModelTitle')
      body = [t('agentSearch.semantic.blocked.noModel')]
      places = ['ai']
      break
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
      places = ['agent']
      break
    case 'failed':
      icon = <TriangleAlert />
      title = t('agentSearch.semantic.blocked.failedTitle')
      body = [t('agentSearch.semantic.blocked.failed')]
      places = ['agent']
      break
    case 'notBuilt':
      icon = <SearchSlash />
      title = t('agentSearch.semantic.blocked.notBuiltTitle')
      body = [t('agentSearch.semantic.blocked.notBuilt')]
      places = ['agent']
      break
    case 'notReady':
      icon = <CircleAlert />
      title = t('agentSearch.semantic.blocked.notReadyTitle')
      body = [t('agentSearch.semantic.blocked.notReady')]
      places = ['agent']
      break
    case 'unknown':
      icon = <TriangleAlert />
      title = t('agentSearch.semantic.blocked.unknownTitle')
      body = [block.error.message]
      places = []
      break
  }
  return (
    <Empty
      role={block.kind === 'unknown' ? 'alert' : 'status'}
      data-semantic-block={block.kind}
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
        {places.map((place) => (
          <Button key={place} asChild size="sm">
            <Link to={place === 'ai' ? AI_SETTINGS : AGENT_SETTINGS}>
              {t('agentSearch.semantic.settings', {
                section: t(sectionName[place]),
              })}
            </Link>
          </Button>
        ))}
        {block.kind === 'unknown' && (
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
