/**
 * What the user searches for and where: the search engines used and the
 * words that come up most. Both count keyword searches in the scope; words
 * link to History, where searching them finds every page they led to.
 */
import { Search } from 'lucide-react'
import { Link } from 'react-router-dom'
import { SectionCard } from '@/components/app/section-card'
import { useFormat, useI18n } from '@/lib/i18n'
import { shareBar } from './display'
import { historyPath } from './links'
import { ListSkeleton, QueryBody, ScrollList, SectionStatus } from './parts'
import { useSearchConcepts, useSearchEngines } from './queries'
import type { Scope } from './range'

export function SearchEnginesCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const engines = useSearchEngines(scope)
  return (
    <SectionCard
      title={t('insights.engines.title')}
      action={
        <SectionStatus
          meta={engines.data?.meta}
          explain={t('insights.engines.explain')}
        />
      }
    >
      <QueryBody
        query={engines}
        skeleton={<ListSkeleton rows={4} />}
        isEmpty={(result) => result.data.length === 0}
        empty={t('insights.searches.empty')}
      >
        {(result) => {
          const total = result.data.reduce(
            (sum, engine) => sum + engine.searchCount,
            0,
          )
          const max = Math.max(1, ...result.data.map((e) => e.searchCount))
          return (
            <ScrollList maxHeight={6 * 38}>
              <ul className="flex flex-col gap-1.5">
                {result.data.map((engine) => (
                  <li
                    key={engine.searchEngine}
                    className="relative flex h-8 items-center gap-2.5 px-2 text-[13px]"
                  >
                    <span
                      aria-hidden
                      className="absolute inset-y-0 left-0 rounded-md bg-blue/15 transition-[width] duration-500"
                      style={shareBar(engine.searchCount, max)}
                    />
                    <Search
                      className="relative size-3.5 text-muted-foreground"
                      aria-hidden
                    />
                    <span className="relative flex-1 truncate font-medium">
                      {engine.displayName || engine.searchEngine}
                    </span>
                    <span className="relative font-mono text-xs text-muted-foreground">
                      {format.compact(engine.searchCount)} ·{' '}
                      {format.percent(total ? engine.searchCount / total : 0)}
                    </span>
                  </li>
                ))}
              </ul>
            </ScrollList>
          )
        }}
      </QueryBody>
    </SectionCard>
  )
}

export function SearchWordsCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const words = useSearchConcepts(scope)
  return (
    <SectionCard
      title={t('insights.words.title')}
      subtitle={t('insights.words.subtitle')}
      action={
        <SectionStatus
          meta={words.data?.meta}
          explain={t('insights.words.explain')}
        />
      }
    >
      <QueryBody
        query={words}
        skeleton={<ListSkeleton rows={3} />}
        isEmpty={(result) => result.data.items.length === 0}
        empty={t('insights.searches.empty')}
      >
        {(result) => (
          <ScrollList maxHeight={5 * 36}>
            <div className="flex flex-wrap gap-1.5">
              {result.data.items.map((word) => (
                <Link
                  key={word.term}
                  to={historyPath({ q: word.term })}
                  className="flex h-7 items-center gap-1.5 rounded-full border bg-card px-3 text-[13px] transition-colors hover:bg-muted hover:no-underline"
                >
                  {word.term}
                  <span className="font-mono text-[11px] text-muted-foreground">
                    {format.compact(word.frequency)}
                  </span>
                </Link>
              ))}
            </div>
          </ScrollList>
        )}
      </QueryBody>
    </SectionCard>
  )
}
