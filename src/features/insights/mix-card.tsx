/**
 * What kinds of sites the scope's visits went to. Each visit counts toward its
 * site's category, from PathKeep's built-in lists of well-known sites
 * (`visit_taxonomy` in vault-core); a range with a previous period of the same
 * length also shows how each share moved, in percentage points. Visits to
 * sites on no list are shown last as not classified, so the shares add up to
 * every visit in the scope; that row opens those sites, to give them a kind on
 * their pages (`unclassified-sites.tsx`).
 */
import { useState } from 'react'
import { SectionCard } from '@/components/app/section-card'
import { cn } from '@/lib/cn'
import type { ActivityMix } from '@/lib/core-intelligence/types'
import { useI18n } from '@/lib/i18n'
import { category, UNCLASSIFIED } from './categories'
import { ShareBar } from './share-bar'
import { ScrollList } from '@/components/app/scroll-list'
import { ListSkeleton, QueryBody, SectionStatus } from './parts'
import { useActivityMix } from './queries'
import { scopeDays, type Scope } from './range'
import { UnclassifiedSites } from './unclassified-sites'
import { useInsightsFormat } from './zone'

export function ActivityMixCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const mix = useActivityMix(scope)
  const [unclassified, setUnclassified] = useState(false)
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
        {(result) => (
          <MixList
            mix={result.data}
            compared={days !== null}
            onUnclassified={() => setUnclassified(true)}
          />
        )}
      </QueryBody>
      <UnclassifiedSites
        scope={scope}
        open={unclassified}
        onOpenChange={setUnclassified}
      />
    </SectionCard>
  )
}

/**
 * Each kind of site with its share of the visits, not classified last; with
 * `compared`, how each share moved from the previous period of the same length.
 * With `onUnclassified`, the not classified row opens the sites it holds.
 */
export function MixList({
  mix,
  compared,
  onUnclassified,
}: {
  mix: ActivityMix
  compared: boolean
  onUnclassified?: () => void
}) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const moved = new Map(
    mix.changeVsPrevious.map((entry) => [
      entry.domainCategory,
      Math.round(entry.changePoints * 100),
    ]),
  )
  const rows = [...mix.categories].sort(
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
          const points = compared ? (moved.get(entry.domainCategory) ?? 0) : 0
          return (
            <li
              key={entry.domainCategory}
              className="relative flex h-8 items-center gap-2.5 px-2 text-[13px]"
            >
              {unclassified && onUnclassified ? (
                <button
                  type="button"
                  onClick={onUnclassified}
                  aria-label={t('insights.mix.unclassified.open')}
                  className="flex-1 truncate text-left text-muted-foreground underline decoration-dotted underline-offset-2 hover:text-foreground"
                >
                  {t('insights.mix.categories.unknown')}
                </button>
              ) : (
                <span
                  className={cn(
                    'flex-1 truncate',
                    unclassified ? 'text-muted-foreground' : 'font-medium',
                  )}
                >
                  {t(
                    `insights.mix.categories.${category(entry.domainCategory)}`,
                  )}
                </span>
              )}
              {points !== 0 && (
                <span className="font-mono text-[11px] text-muted-foreground">
                  {t('insights.mix.points', {
                    value: `${points > 0 ? '+' : '−'}${format.number(Math.abs(points))}`,
                  })}
                </span>
              )}
              <ShareBar
                value={entry.visitCount}
                max={max}
                tone={unclassified ? 'muted' : 'blue'}
              />
              <span className="w-14 text-right font-mono text-xs text-muted-foreground">
                {format.percent(entry.visitCount / total)}
              </span>
            </li>
          )
        })}
      </ul>
    </ScrollList>
  )
}
