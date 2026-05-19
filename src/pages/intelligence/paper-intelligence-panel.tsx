/**
 * Paper-redesign panel that maps the existing primary-overview + dashboard
 * data into PaperIntelligenceView. Mounted alongside the v0.2 sections when
 * the route has `?layout=paper`, so the redesign can be QA'd inline without
 * disturbing the existing layout.
 *
 * ## Responsibilities
 * - Project `primaryOverview.topSites` → PaperDomainRankRow[].
 * - Project `primaryOverview.refindPages` → PaperRefindItem[].
 * - Compose 4 KPI cells from the dashboard snapshot + the top domain.
 *
 * ## Not responsible for
 * - Fetching primaryOverview or dashboard — the route still owns the
 *   read models.
 * - Topic / session / thread surfaces — those are deliberately left empty
 *   in this pass and PaperIntelligenceView renders the empty branches.
 */

import { useMemo } from 'react'
import { PaperIntelligenceView } from '@/components/explorer-paper'
import { getDomainAbbr, getDomainColor } from '../explorer/paper/domain-color'
import { buildPaperIntelligenceCopy } from '../explorer/paper-explorer-copy'
import type { peekIntelligencePrimaryOverview } from '../../lib/core-intelligence'
import type { useShellData } from '../../app/shell-data-context'

export interface PaperIntelligencePanelProps {
  primaryOverview: ReturnType<typeof peekIntelligencePrimaryOverview>
  dashboard: ReturnType<typeof useShellData>['dashboard']
  onSelectDomain: (domain: string) => void
  explorerT: (key: string, vars?: Record<string, string | number>) => string
}

export function PaperIntelligencePanel({
  primaryOverview,
  dashboard,
  onSelectDomain,
  explorerT,
}: PaperIntelligencePanelProps) {
  const copy = useMemo(() => buildPaperIntelligenceCopy(explorerT), [explorerT])

  const domains = useMemo(
    () =>
      (primaryOverview?.topSites.data ?? []).slice(0, 8).map((site) => ({
        domain: site.registrableDomain,
        count: site.visitCount,
      })),
    [primaryOverview?.topSites.data],
  )

  const refindItems = useMemo(
    () =>
      (primaryOverview?.refindPages.data ?? []).slice(0, 6).map((page) => ({
        id: page.canonicalUrl,
        title: page.title ?? page.url,
        domain: page.registrableDomain,
        meta: `${page.crossDayCount} days · ${page.trailCount} sessions`,
      })),
    [primaryOverview?.refindPages.data],
  )

  const kpis = useMemo(
    () => [
      {
        id: 'week',
        label: explorerT('paperIntelligence.kpiThisWeekLabel'),
        value: (dashboard?.totalVisits ?? 0).toLocaleString(),
      },
      {
        id: 'top',
        label: explorerT('paperIntelligence.kpiTopDomainLabel'),
        value: domains[0]?.domain ?? '—',
        monoValue: true,
        sub:
          domains[0]?.count !== undefined
            ? explorerT('paperIntelligence.kpiTopDomainSub', {
                count: domains[0].count,
                pct: Math.round(
                  (domains[0].count /
                    Math.max(
                      1,
                      domains.reduce((acc, row) => acc + row.count, 0),
                    )) *
                    100,
                ),
              })
            : undefined,
      },
      {
        id: 'threads',
        label: explorerT('paperIntelligence.kpiActiveThreadsLabel'),
        value: String(refindItems.length),
      },
      {
        id: 'sources',
        label: 'Sources',
        value: String(dashboard?.recentRuns.length ?? 0),
      },
    ],
    [dashboard, domains, refindItems, explorerT],
  )

  return (
    <section
      data-testid="paper-intelligence-panel"
      className="border-border-light mt-6 border-t pt-6"
    >
      <PaperIntelligenceView
        kpis={kpis}
        topics={[]}
        domains={domains}
        sessions={[]}
        threads={[]}
        refindItems={refindItems}
        resolveDomainColor={getDomainColor}
        resolveDomainAbbr={getDomainAbbr}
        onSelectDomain={onSelectDomain}
        copy={copy}
        testId="paper-intelligence-view"
      />
    </section>
  )
}
