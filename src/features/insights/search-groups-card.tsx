/**
 * Responsibilities: show the user's search subjects and the real days they searched each.
 * Not responsible for: grouping, month caching or deciding serving-index eligibility.
 * Failure modes: scope is lost on drill-in, a stale month looks empty, or years grow the DOM.
 * Tests: semantic-index.spec.ts (counts, days, months, profile, service removal, screenshots).
 */
import { useRef } from 'react'
import type { UseQueryResult } from '@tanstack/react-query'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Link } from 'react-router-dom'
import { SectionCard } from '@/components/app/section-card'
import { ScrollList } from '@/components/app/scroll-list'
import { useI18n } from '@/lib/i18n'
import {
  type SearchGroup,
  type SearchGroupWindow,
  type SearchGroupsCard as SearchGroupsData,
} from '@/lib/backend-client/search-groups'
import { addDays, daysBetween } from '@/lib/time-zone'
import { cn } from '@/lib/cn'
import { searchPath } from './links'
import { ListSkeleton, QueryBody, SectionStatus } from './parts'
import type { Scope } from './range'
import { useInsightsFormat } from './zone'

/** The optional card occupies one grid cell; its deterministic neighbours retain their layout. */
export function SearchGroupsCard({
  scope,
  query,
}: {
  scope: Scope
  query: UseQueryResult<SearchGroupsData>
}) {
  const { t } = useI18n()
  const monthly = daysBetween(scope.dates.start, scope.dates.end) >= 31
  const range =
    scope.range === 'all'
      ? t('insights.groups.all')
      : scope.range === 'custom'
        ? `${scope.dates.start} – ${scope.dates.end}`
        : t('insights.groups.recent', {
            days: daysBetween(scope.dates.start, scope.dates.end) + 1,
          })
  return (
    <SectionCard
      title={t('insights.groups.title')}
      subtitle={
        <>
          {t(monthly ? 'insights.groups.monthly' : 'insights.groups.range', {
            range,
          })}
          {monthly && (
            <span className="block">{t('insights.groups.wholeMonths')}</span>
          )}
          {monthly && scope.profileId && (
            <span className="block">{t('insights.groups.allBrowsers')}</span>
          )}
          {query.data?.staticModel && (
            <Link
              to="/settings/ai"
              className="block underline underline-offset-2"
            >
              {t('insights.groups.static')}
            </Link>
          )}
        </>
      }
      action={
        <SectionStatus
          meta={query.data?.meta}
          explain={t('insights.groups.explain')}
        />
      }
    >
      <QueryBody
        query={query}
        skeleton={
          <div role="status" aria-label={t('insights.groups.loading')}>
            <ListSkeleton rows={4} height={70} />
          </div>
        }
        isEmpty={(result) =>
          !result.data.some((window) => window.updating || window.groups.length)
        }
        empty={t('insights.groups.empty')}
      >
        {(result) =>
          result.monthly ? (
            <Months windows={result.data} scope={scope} />
          ) : (
            <ScrollList maxHeight={460}>
              <GroupWindow window={result.data[0]} scope={scope} />
            </ScrollList>
          )
        }
      </QueryBody>
    </SectionCard>
  )
}

/** Months are virtualized even for all time; ScrollList keeps its visible overflow fade. */
function Months({
  windows,
  scope,
}: {
  windows: SearchGroupWindow[]
  scope: Scope
}) {
  const content = useRef<HTMLDivElement>(null)
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: windows.length,
    getScrollElement: () => content.current?.parentElement ?? null,
    estimateSize: () => 310,
    getItemKey: (index) => windows[index].range.start,
    overscan: 1,
  })
  const format = useInsightsFormat()
  return (
    <ScrollList maxHeight={460}>
      <div
        ref={content}
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((item) => {
          const window = windows[item.index]
          return (
            <div
              key={item.key}
              data-index={item.index}
              ref={virtualizer.measureElement}
              className="absolute top-0 left-0 w-full pb-5"
              style={{ transform: `translateY(${item.start}px)` }}
              data-search-month={window.range.start.slice(0, 7)}
            >
              <h3 className="mb-2 text-[13px] font-semibold">
                {format.day(window.range.start, {
                  month: 'long',
                  year: 'numeric',
                })}
              </h3>
              <GroupWindow window={window} scope={scope} />
            </div>
          )
        })}
      </div>
    </ScrollList>
  )
}

function GroupWindow({
  window,
  scope,
}: {
  window?: SearchGroupWindow
  scope: Scope
}) {
  const { t } = useI18n()
  if (!window || window.updating)
    return (
      <div
        role="status"
        className="flex flex-col gap-2 text-xs text-muted-foreground"
      >
        {t('insights.groups.updating')}
        <ListSkeleton rows={3} height={60} />
      </div>
    )
  if (!window.groups.length)
    return (
      <p className="text-xs text-muted-foreground">
        {t('insights.groups.empty')}
      </p>
    )
  return (
    <ul className="flex flex-col gap-3">
      {window.groups.map((group) => (
        <GroupRow
          key={group.wordings[0]}
          group={group}
          window={window}
          scope={scope}
        />
      ))}
    </ul>
  )
}

function GroupRow({
  group,
  window,
  scope,
}: {
  group: SearchGroup
  window: SearchGroupWindow
  scope: Scope
}) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const dates = Array.from(
    { length: daysBetween(window.range.start, window.range.end) + 1 },
    (_, day) => addDays(window.range.start, day),
  )
  const active = new Set(group.days)
  return (
    <li data-search-group={group.wordings[0]}>
      <Link
        to={searchPath(group.wordings[0], scope)}
        className="-mx-2 flex flex-col gap-1.5 rounded-md px-2 py-1 text-[13px] transition-colors hover:bg-muted hover:no-underline"
      >
        <span className="line-clamp-2 break-words">
          <span className="font-medium">{group.wordings[0]}</span>
          <span className="text-muted-foreground">
            {group.wordings
              .slice(1, 3)
              .map((wording) => ` · ${wording}`)
              .join('')}
          </span>
        </span>
        {group.distinctSearches > 3 && (
          <span className="text-xs text-muted-foreground">
            {t('insights.groups.others', { count: group.distinctSearches - 3 })}
          </span>
        )}
        <span className="text-xs text-muted-foreground tabular">
          {[
            t('insights.groups.searchCount', { count: group.searches }),
            t('insights.groups.dayCount', { count: group.days.length }),
            t('insights.groups.pageCount', { count: group.pagesOpened }),
          ].join(' · ')}
        </span>
        <span
          className="flex gap-0.5"
          role="group"
          aria-label={t('insights.groups.strip')}
        >
          {dates.map((date) => (
            <span
              key={date}
              role="img"
              data-date={date}
              data-searched={active.has(date)}
              title={format.day(date)}
              aria-label={t(
                active.has(date)
                  ? 'insights.groups.searched'
                  : 'insights.groups.notSearched',
                {
                  date: format.day(date),
                },
              )}
              className={cn(
                'h-2 min-w-0 flex-1 rounded-[2px]',
                active.has(date) ? 'bg-heat-4' : 'bg-heat-0',
              )}
            />
          ))}
        </span>
      </Link>
    </li>
  )
}
