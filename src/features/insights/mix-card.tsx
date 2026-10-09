/**
 * What kinds of sites the scope's visits went to. Each visit counts toward its
 * site's category, from PathKeep's built-in lists of well-known sites
 * (`visit_taxonomy` in vault-core); a range with a previous period of the same
 * length also shows how each share moved, in percentage points. Visits to
 * sites on no list are shown last as not classified, so the shares add up to
 * every visit in the scope.
 */
import { SectionCard } from '@/components/app/section-card'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { shareBar } from './display'
import { ListSkeleton, QueryBody, ScrollList, SectionStatus } from './parts'
import { useActivityMix } from './queries'
import { scopeDays, type Scope } from './range'

const UNCLASSIFIED = 'unknown'
const CATEGORIES = [
  'ai',
  'community',
  'developer',
  'docs',
  'education',
  'entertainment',
  'finance',
  'news',
  'search',
  'shopping',
  'social',
  'travel',
  'video',
  'work',
  UNCLASSIFIED,
] as const
type Category = (typeof CATEGORIES)[number]

/** The backend's category, or not classified for one this version has no name for. */
function category(name: string): Category {
  return CATEGORIES.find((known) => known === name) ?? UNCLASSIFIED
}

export function ActivityMixCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const mix = useActivityMix(scope)
  // All time has no previous period to compare with.
  const days = scopeDays(scope)
  const explain = days
    ? `${t('insights.mix.explain')} ${t('insights.mix.compared', { days })}`
    : t('insights.mix.explain')

  return (
    <SectionCard
      title={t('insights.mix.title')}
      action={<SectionStatus meta={mix.data?.meta} explain={explain} />}
    >
      <QueryBody
        query={mix}
        skeleton={<ListSkeleton rows={5} />}
        isEmpty={(result) => result.data.categories.length === 0}
        empty={t('insights.topSites.empty')}
      >
        {(result) => {
          const moved = new Map(
            result.data.changeVsPrevious.map((entry) => [
              entry.domainCategory,
              Math.round(entry.changePoints * 100),
            ]),
          )
          const rows = [...result.data.categories].sort(
            (a, b) =>
              Number(a.domainCategory === UNCLASSIFIED) -
                Number(b.domainCategory === UNCLASSIFIED) ||
              b.visitCount - a.visitCount,
          )
          const max = Math.max(1, ...rows.map((entry) => entry.visitCount))
          const total = rows.reduce((sum, entry) => sum + entry.visitCount, 0)
          return (
            <ScrollList maxHeight={6 * 38}>
              <ul className="flex flex-col gap-1.5">
                {rows.map((entry) => {
                  const unclassified = entry.domainCategory === UNCLASSIFIED
                  const points = days
                    ? (moved.get(entry.domainCategory) ?? 0)
                    : 0
                  return (
                    <li
                      key={entry.domainCategory}
                      className="relative flex h-8 items-center gap-2.5 px-2 text-[13px]"
                    >
                      <span
                        aria-hidden
                        className={cn(
                          'absolute inset-y-0 left-0 rounded-md transition-[width] duration-500',
                          unclassified ? 'bg-muted' : 'bg-blue/15',
                        )}
                        style={shareBar(entry.visitCount, max)}
                      />
                      <span
                        className={cn(
                          'relative flex-1 truncate',
                          unclassified
                            ? 'text-muted-foreground'
                            : 'font-medium',
                        )}
                      >
                        {t(
                          `insights.mix.categories.${category(entry.domainCategory)}`,
                        )}
                      </span>
                      {points !== 0 && (
                        <span className="relative font-mono text-[11px] text-muted-foreground">
                          {t('insights.mix.points', {
                            value: `${points > 0 ? '+' : '−'}${format.number(Math.abs(points))}`,
                          })}
                        </span>
                      )}
                      <span className="relative w-14 text-right font-mono text-xs text-muted-foreground">
                        {format.percent(entry.visitCount / total)}
                      </span>
                    </li>
                  )
                })}
              </ul>
            </ScrollList>
          )
        }}
      </QueryBody>
    </SectionCard>
  )
}
