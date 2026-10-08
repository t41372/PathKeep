/**
 * What the user searches for, where, and how it goes: the search engines
 * used, the words that come up most, how often searches are reworded and how
 * many pages they open, the searches come back to most, and the ones that
 * took several tries. All count searches in the scope; searches link to their
 * drill-in, words to History.
 */
import { RotateCcw, Search } from 'lucide-react'
import { Link } from 'react-router-dom'
import { Favicon } from '@/components/app/favicon'
import { SectionCard } from '@/components/app/section-card'
import { useFormat, useI18n } from '@/lib/i18n'
import { shareBar } from './display'
import { historyPath, searchPath, sitePath } from './links'
import { ListSkeleton, QueryBody, ScrollList, SectionStatus } from './parts'
import { rowLink } from './display'
import {
  useFriction,
  useSearchConcepts,
  useSearchEffectiveness,
  useSearchEngines,
  useSearchLandings,
} from './queries'
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

/**
 * How searches go on each engine (how often a search is reworded before it
 * settles, how many pages it opens), and the searches come back to most, over
 * the longest stretch.
 */
export function SearchOutcomesCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const outcomes = useSearchEffectiveness(scope)
  const decimal = (value: number) => format.number(Math.round(value * 10) / 10)
  return (
    <SectionCard
      title={t('insights.outcomes.title')}
      action={
        <SectionStatus
          meta={outcomes.data?.meta}
          explain={t('insights.outcomes.explain')}
        />
      }
    >
      <QueryBody
        query={outcomes}
        skeleton={<ListSkeleton rows={4} />}
        isEmpty={(result) => result.data.engineStats.length === 0}
        empty={t('insights.searches.empty')}
      >
        {(result) => (
          <div className="flex flex-col gap-4">
            <ul className="flex flex-col gap-2 text-[13px]">
              {result.data.engineStats.slice(0, 4).map((engine) => (
                <li key={engine.searchEngine} className="flex flex-col gap-0.5">
                  <span className="flex items-baseline justify-between gap-2">
                    <span className="truncate font-medium">
                      {engine.displayName || engine.searchEngine}
                    </span>
                    <span className="shrink-0 text-xs text-muted-foreground tabular">
                      {t('insights.outcomes.searches', {
                        count: engine.totalTrails,
                        searches: format.number(engine.totalTrails),
                      })}
                    </span>
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {t('insights.outcomes.engine', {
                      rewordings: decimal(engine.avgReformulations),
                      pages: decimal(engine.avgDepth),
                    })}
                  </span>
                </li>
              ))}
            </ul>
            {result.data.hardestTopics.items.length > 0 && (
              <div className="flex flex-col gap-1.5">
                <h3 className="text-[13px] font-semibold">
                  {t('insights.outcomes.mostReturned')}
                </h3>
                <ul className="-mx-2 flex flex-col">
                  {result.data.hardestTopics.items.map((topic) => (
                    <li key={topic.familyId}>
                      <Link
                        to={searchPath(topic.queryFamily, scope)}
                        className={rowLink}
                      >
                        <Search
                          className="size-3.5 shrink-0 text-muted-foreground"
                          aria-hidden
                        />
                        <span className="flex-1 truncate">
                          {topic.queryFamily}
                        </span>
                        <span className="text-xs text-muted-foreground tabular">
                          {t('insights.outcomes.topic', {
                            count: topic.reformulationCount,
                            searches: format.number(topic.reformulationCount),
                            days: format.number(
                              Math.max(1, Math.round(topic.reSearchLagDays)),
                            ),
                          })}
                        </span>
                      </Link>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}
      </QueryBody>
    </SectionCard>
  )
}

/** Searches reworded twice or more, or that never settled on a page. */
export function SearchTriesCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const friction = useFriction(scope)
  return (
    <SectionCard
      title={t('insights.tries.title')}
      subtitle={t('insights.tries.subtitle')}
      action={
        <SectionStatus
          meta={friction.data?.meta}
          explain={t('insights.tries.explain')}
        />
      }
    >
      <QueryBody
        query={friction}
        skeleton={<ListSkeleton rows={5} height={40} />}
        isEmpty={(result) => result.data.items.length === 0}
        empty={t('insights.tries.empty')}
      >
        {(result) => (
          <ScrollList maxHeight={6 * 44}>
            <ul className="-mx-2 flex flex-col">
              {result.data.items.map((signal, index) => (
                <li key={`${signal.searchQuery}-${index}`}>
                  <Link
                    to={searchPath(signal.searchQuery, scope)}
                    className={rowLink}
                  >
                    <RotateCcw
                      className="size-3.5 shrink-0 text-muted-foreground"
                      aria-hidden
                    />
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span className="truncate">{signal.searchQuery}</span>
                      <span className="truncate text-xs text-muted-foreground">
                        {[
                          signal.reformulationCount > 0 &&
                            t('insights.tries.reworded', {
                              count: signal.reformulationCount,
                              times: format.number(signal.reformulationCount),
                            }),
                          signal.registrableDomain
                            ? t('insights.tries.endedOn', {
                                site: signal.registrableDomain,
                              })
                            : t('insights.tries.noResult'),
                        ]
                          .filter(Boolean)
                          .join(' · ')}
                      </span>
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          </ScrollList>
        )}
      </QueryBody>
    </SectionCard>
  )
}

/** The sites searches ended on: where the user's answers come from. */
export function SearchLandingsCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const landings = useSearchLandings(scope)
  return (
    <SectionCard
      title={t('insights.landings.title')}
      subtitle={t('insights.landings.subtitle')}
      action={
        <SectionStatus
          meta={landings.data?.meta}
          explain={t('insights.landings.explain')}
        />
      }
    >
      <QueryBody
        query={landings}
        skeleton={<ListSkeleton rows={6} />}
        isEmpty={(result) => result.data.items.length === 0}
        empty={t('insights.searches.empty')}
      >
        {(result) => {
          const max = Math.max(
            1,
            ...result.data.items.map((s) => s.searchCount),
          )
          return (
            <ScrollList maxHeight={6 * 38}>
              <ul className="flex flex-col gap-1.5">
                {result.data.items.map((landing) => (
                  <li key={landing.registrableDomain}>
                    <Link
                      to={sitePath(landing.registrableDomain, scope)}
                      className="relative flex h-8 items-center gap-2.5 rounded-md px-2 text-[13px] hover:no-underline"
                    >
                      <span
                        aria-hidden
                        className="absolute inset-y-0 left-0 rounded-md bg-green/15 transition-[width] duration-500"
                        style={shareBar(landing.searchCount, max)}
                      />
                      <Favicon
                        domain={landing.registrableDomain}
                        className="relative"
                      />
                      <span className="relative flex-1 truncate font-medium">
                        {landing.registrableDomain}
                      </span>
                      <span className="relative text-xs text-muted-foreground tabular">
                        {t('insights.landings.counts', {
                          count: landing.searchCount,
                          searches: format.compact(landing.searchCount),
                          queries: format.number(landing.queryCount),
                        })}
                      </span>
                    </Link>
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
