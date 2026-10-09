/**
 * The pattern cards below the first Insights cards: threads you came back
 * to, explore or focus, habits, and browsers compared. Each is one backend
 * aggregate over the range and links into a drill-in.
 *
 * Not built, because the backend has nothing honest to show yet (see
 * docs/features/intelligence.md §0): open loops, search effectiveness and
 * friction, source roles.
 */
import { FileText, Search } from 'lucide-react'
import { Link } from 'react-router-dom'
import { BrowserIcon } from '@/lib/browser-icons'
import { SiteIcon } from '@/features/history/site-icon'
import { SectionCard } from '@/components/app/section-card'
import { Skeleton } from '@/components/ui/skeleton'
import { useSourceStats } from '@/features/home/queries'
import { cn } from '@/lib/cn'
import type { HabitPattern } from '@/lib/core-intelligence/types'
import { useFormat, useI18n } from '@/lib/i18n'
import { profileLabel } from '@/lib/profile-label'
import { pagePath, searchPath, sitePath } from './links'
import { category, UNCLASSIFIED } from './categories'
import { rowLink } from './display'
import { ListSkeleton, QueryBody, SectionStatus } from './parts'
import {
  useBreadth,
  useBrowsers,
  useDigest,
  useHabits,
  useThreads,
} from './queries'
import type { Scope } from './range'

export function ThreadsCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const threads = useThreads(scope)
  return (
    <SectionCard
      title={t('insights.threads.title')}
      subtitle={t('insights.threads.subtitle')}
      action={
        <SectionStatus
          meta={threads.data?.meta}
          explain={t('insights.threads.explain')}
        />
      }
    >
      <QueryBody
        query={threads}
        skeleton={<ListSkeleton rows={4} height={40} />}
        isEmpty={(result) => result.data.length === 0}
        empty={t('insights.threads.empty')}
      >
        {(result) => (
          <ul className="-mx-2 flex flex-col">
            {result.data.map((thread) => {
              const isSearch = thread.anchorType === 'query_family'
              const Icon = isSearch ? Search : FileText
              return (
                <li key={thread.investigationId}>
                  <Link
                    to={
                      isSearch
                        ? searchPath(thread.anchorLabel, scope)
                        : pagePath(thread.anchorId, scope)
                    }
                    className={cn(rowLink, 'items-start py-2')}
                  >
                    <Icon
                      className="mt-0.5 size-3.5 shrink-0 text-muted-foreground"
                      aria-hidden
                    />
                    <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                      <span className="truncate font-medium">
                        {thread.anchorLabel}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {isSearch
                          ? t('insights.threads.searched', {
                              count: thread.occurrenceCount,
                              times: format.number(thread.occurrenceCount),
                            })
                          : t('insights.threads.opened', {
                              count: thread.distinctDays,
                              days: format.number(thread.distinctDays),
                            })}
                        {' · '}
                        {t('insights.threads.last', {
                          when: format.relative(thread.lastSeenAt),
                        })}
                      </span>
                    </span>
                  </Link>
                </li>
              )
            })}
          </ul>
        )}
      </QueryBody>
    </SectionCard>
  )
}

/** Where the breadth score falls, in words. */
function breadthWord(score: number) {
  if (score >= 75) return 'wide' as const
  if (score >= 45) return 'mixed' as const
  return 'focused' as const
}

export function BreadthCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const breadth = useBreadth(scope)
  const digest = useDigest(scope)
  const newSites = digest.data?.data.newDomains.value

  return (
    <SectionCard
      title={t('insights.breadth.title')}
      action={
        <SectionStatus
          meta={breadth.data?.meta}
          explain={t('insights.breadth.explain')}
        />
      }
    >
      <QueryBody
        query={breadth}
        skeleton={<Skeleton className="h-28" />}
        isEmpty={(result) => result.data.concentrationDomainCount === 0}
        empty={t('insights.topSites.empty')}
      >
        {(result) => {
          const score = Math.round(result.data.breadthScore)
          const word = breadthWord(score)
          return (
            <div className="flex flex-col gap-4">
              <p className="text-[15px] leading-snug">
                {t('insights.breadth.half', {
                  count: result.data.concentrationDomainCount,
                  sites: format.number(result.data.concentrationDomainCount),
                })}
              </p>
              <div className="flex flex-col gap-1.5">
                <div
                  role="meter"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={score}
                  aria-label={t('insights.breadth.meter')}
                  className="relative h-2 rounded-full bg-gradient-to-r from-blue/30 via-muted to-brand/40"
                >
                  <span
                    className="absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-card bg-foreground shadow-card transition-[left] duration-500"
                    style={{ left: `${score}%` }}
                  />
                </div>
                <div className="flex justify-between text-[11px] text-muted-foreground">
                  <span>{t('insights.breadth.focusedEnd')}</span>
                  <span>{t('insights.breadth.wideEnd')}</span>
                </div>
              </div>
              <p className="text-[13px] text-muted-foreground">
                {t(`insights.breadth.${word}`)}
                {/* Over all time every site was new once; the count says nothing. */}
                {scope.range !== 'all' &&
                  newSites != null &&
                  newSites > 0 &&
                  ` ${t('insights.breadth.newSites', {
                    count: newSites,
                    sites: format.number(newSites),
                  })}`}
              </p>
            </div>
          )
        }}
      </QueryBody>
    </SectionCard>
  )
}

function habitRhythm(habit: HabitPattern, t: ReturnType<typeof useI18n>['t']) {
  switch (habit.habitType) {
    case 'daily_habit':
      return t('insights.habits.daily')
    case 'weekly_habit':
      return t('insights.habits.weekly')
    default:
      return t('insights.habits.periodic', {
        days: String(Math.round(habit.meanIntervalDays)),
      })
  }
}

export function HabitsCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const habits = useHabits(scope)
  return (
    <SectionCard
      title={t('insights.habits.title')}
      subtitle={t('insights.habits.subtitle')}
      action={
        <SectionStatus
          meta={habits.data?.meta}
          explain={t('insights.habits.explain')}
        />
      }
    >
      <QueryBody
        query={habits}
        skeleton={<ListSkeleton rows={4} height={40} />}
        isEmpty={(result) => result.data.length === 0}
        empty={t('insights.habits.empty')}
      >
        {(result) => (
          <ul className="-mx-2 flex flex-col">
            {result.data.map((habit) => (
              <li key={habit.registrableDomain}>
                <Link
                  to={sitePath(habit.registrableDomain, scope)}
                  className={cn(rowLink, 'py-2')}
                >
                  <SiteIcon domain={habit.registrableDomain} />
                  <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span className="truncate font-medium">
                      {habit.registrableDomain}
                    </span>
                    <span className="text-xs text-muted-foreground">
                      {habitRhythm(habit, t)} ·{' '}
                      {t('insights.habits.days', {
                        count: habit.visitCount,
                        days: format.number(habit.visitCount),
                      })}
                    </span>
                  </span>
                  {habit.isInterrupted ? (
                    <span className="rounded-md bg-brand-soft px-1.5 py-0.5 text-[11px] text-brand">
                      {t('insights.habits.stopped', {
                        when: format.relative(habit.lastVisitedAt),
                      })}
                    </span>
                  ) : (
                    <span className="text-xs text-muted-foreground">
                      {format.relative(habit.lastVisitedAt)}
                    </span>
                  )}
                </Link>
              </li>
            ))}
          </ul>
        )}
      </QueryBody>
    </SectionCard>
  )
}

export function BrowsersCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const browsers = useBrowsers(scope)
  const sources = useSourceStats()
  const names = new Map(
    (sources.data ?? []).map((source) => [source.profileId, source]),
  )
  return (
    <SectionCard
      title={t('insights.browsers.title')}
      action={
        <SectionStatus
          meta={browsers.data?.meta}
          explain={t('insights.browsers.explain')}
        />
      }
    >
      <QueryBody
        query={browsers}
        skeleton={<ListSkeleton rows={3} height={40} />}
        isEmpty={(result) => result.data.profiles.length < 2}
        empty={t('insights.browsers.single')}
      >
        {(result) => {
          const {
            profiles,
            sharedDomains,
            exclusiveDomains,
            categoryDistributions,
          } = result.data
          const total = profiles.reduce(
            (sum, profile) => sum + profile.visitCount,
            0,
          )
          const ordered = [...profiles].sort(
            (a, b) => b.visitCount - a.visitCount,
          )
          return (
            <div className="flex flex-col gap-3">
              <ul className="flex flex-col gap-2.5">
                {ordered.map((profile) => {
                  const source = names.get(profile.profileId)
                  const only = exclusiveDomains
                    .filter((entry) => entry.profileId === profile.profileId)
                    .sort((a, b) => b.visitCount - a.visitCount)
                    .slice(0, 3)
                  // Its two biggest kinds of sites, not counting unclassified ones.
                  const kinds = (
                    categoryDistributions.find(
                      (entry) => entry.profileId === profile.profileId,
                    )?.categories ?? []
                  )
                    .filter((entry) => entry.domainCategory !== UNCLASSIFIED)
                    .sort((a, b) => b.visitCount - a.visitCount)
                    .slice(0, 2)
                  return (
                    <li key={profile.profileId} className="flex flex-col gap-1">
                      <span className="flex items-baseline justify-between gap-2 text-[13px]">
                        <span className="flex min-w-0 items-center gap-1.5 font-medium">
                          {source && (
                            <BrowserIcon
                              browserName={source.browserName}
                              decorative
                              className="size-4 shrink-0"
                            />
                          )}
                          <span className="truncate">
                            {source
                              ? profileLabel(source, t)
                              : profile.profileName}
                          </span>
                        </span>
                        <span className="shrink-0 text-xs text-muted-foreground tabular">
                          {t('common.visits', { count: profile.visitCount })} ·{' '}
                          {t('common.sites', { count: profile.domainCount })}
                        </span>
                      </span>
                      <span className="h-1.5 overflow-hidden rounded-full bg-muted">
                        <span
                          className="block h-full rounded-full bg-brand transition-[width] duration-500"
                          style={{
                            width: `${total ? (profile.visitCount / total) * 100 : 0}%`,
                          }}
                        />
                      </span>
                      {kinds.length > 0 && (
                        <span className="truncate text-xs text-muted-foreground">
                          {t('insights.browsers.kinds', {
                            kinds: kinds
                              .map(
                                (entry) =>
                                  `${t(`insights.mix.categories.${category(entry.domainCategory)}`)} ${format.percent(entry.share)}`,
                              )
                              .join(t('insights.browsers.kindsJoin')),
                          })}
                        </span>
                      )}
                      {only.length > 0 && (
                        <span className="truncate text-xs text-muted-foreground">
                          {t('insights.browsers.only')}{' '}
                          {only.map((entry, index) => (
                            <span key={entry.registrableDomain}>
                              {index > 0 && ', '}
                              <Link
                                to={sitePath(entry.registrableDomain, scope)}
                                className="text-foreground hover:underline"
                              >
                                {entry.registrableDomain}
                              </Link>
                            </span>
                          ))}
                        </span>
                      )}
                    </li>
                  )
                })}
              </ul>
              <p className="text-xs text-muted-foreground">
                {t('insights.browsers.shared', {
                  count: sharedDomains.length,
                  sites: format.number(sharedDomains.length),
                })}
                {sharedDomains.length > 0 && (
                  <>
                    {' '}
                    {t('insights.browsers.sharedTop')}{' '}
                    {sharedDomains.slice(0, 3).map((domain, index) => (
                      <span key={domain}>
                        {index > 0 && t('insights.browsers.kindsJoin')}
                        <Link
                          to={sitePath(domain, scope)}
                          className="text-foreground hover:underline"
                        >
                          {domain}
                        </Link>
                      </span>
                    ))}
                  </>
                )}
              </p>
            </div>
          )
        }}
      </QueryBody>
    </SectionCard>
  )
}
