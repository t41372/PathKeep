/**
 * PaperIntelligenceView — composed Intelligence shell.
 *
 * Mirrors `pk-intelligence.jsx` IntelligenceView: 4-cell KPI strip,
 * "Topics over the last N days" card with axis + optional LLM summary,
 * a 2-column grid below with Where you spent your time (top domains) +
 * Recent sessions on the left and Active threads + Refind candidates on
 * the right.
 *
 * ## Responsibilities
 * - Compose the Intelligence primitives into the route's reading order.
 * - Surface section title copy via the props bundle.
 * - Route every interaction (domain click, session click, thread click,
 *   refind click) to the caller-supplied handlers.
 *
 * ## Not responsible for
 * - Data fetching — caller maps the core-intelligence read models into
 *   the typed shape this view expects.
 * - Drill-in URL composition — the consumer's `onSelect*` handlers do
 *   that work.
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { PaperKpiStrip, type PaperKpiCell } from './paper-kpi-strip'
import { PaperTopicTimeline, type PaperTopicRow } from './paper-topic-timeline'
import {
  PaperDomainRankList,
  type PaperDomainRankRow,
} from './paper-domain-rank'
import { PaperThreadList, type PaperThreadRow } from './paper-thread-list'
import { PaperRefindShelf, type PaperRefindItem } from './paper-refind-shelf'

export interface PaperIntelligenceViewCopy {
  kpisLabel?: string
  topicsTitle: string
  topicsRangeBadge: string
  /** Optional italic-serif summary line beneath the topic timeline. */
  topicsSummary?: ReactNode
  domainsTitle: string
  domainsBadge?: string
  sessionsTitle: string
  sessionsBadge?: string
  threadsTitle: string
  refindTitle: string
  refindBadge?: string
  sessionPagesLabel?: string
  threadPagesLabel?: string
}

export interface PaperIntelligenceViewProps {
  kpis: readonly PaperKpiCell[]
  topics: readonly PaperTopicRow[]
  topicAxisLabels?: readonly string[]
  domains: readonly PaperDomainRankRow[]
  sessions: readonly PaperThreadRow[]
  threads: readonly PaperThreadRow[]
  refindItems: readonly PaperRefindItem[]
  resolveDomainColor: (domain: string) => string
  resolveDomainAbbr: (domain: string) => string
  onSelectDomain?: (domain: string) => void
  onSelectSession?: (session: PaperThreadRow) => void
  onSelectThread?: (thread: PaperThreadRow) => void
  onSelectRefind?: (item: PaperRefindItem) => void
  copy: PaperIntelligenceViewCopy
  className?: string
  testId?: string
}

export function PaperIntelligenceView({
  kpis,
  topics,
  topicAxisLabels,
  domains,
  sessions,
  threads,
  refindItems,
  resolveDomainColor,
  resolveDomainAbbr,
  onSelectDomain,
  onSelectSession,
  onSelectThread,
  onSelectRefind,
  copy,
  className,
  testId,
}: PaperIntelligenceViewProps) {
  return (
    <section
      data-testid={testId}
      className={cn('flex w-full flex-col', className)}
    >
      <PaperKpiStrip cells={kpis} testId="paper-intelligence-kpis" />

      <PaperCard className="mb-5">
        <PaperCardHeader
          title={copy.topicsTitle}
          right={<span>{copy.topicsRangeBadge}</span>}
        />
        <PaperCardBody>
          <PaperTopicTimeline
            rows={topics}
            axisLabels={topicAxisLabels}
            testId="paper-intelligence-topic-timeline"
          />
          {copy.topicsSummary ? (
            <div
              data-testid="paper-intelligence-topics-summary"
              className="border-border-light text-ink-muted mt-4 border-t border-dashed pt-3 font-serif text-[13px] italic leading-[1.5]"
            >
              {copy.topicsSummary}
            </div>
          ) : null}
        </PaperCardBody>
      </PaperCard>

      <div className="grid gap-5 lg:grid-cols-[1fr_360px]">
        <div className="flex flex-col gap-5">
          <PaperCard>
            <PaperCardHeader
              title={copy.domainsTitle}
              right={
                copy.domainsBadge ? <span>{copy.domainsBadge}</span> : undefined
              }
            />
            <PaperCardBody className="px-[18px] py-[14px]">
              <PaperDomainRankList
                rows={domains}
                onSelectDomain={onSelectDomain}
                testId="paper-intelligence-domain-rank"
              />
            </PaperCardBody>
          </PaperCard>

          <PaperCard>
            <PaperCardHeader
              title={copy.sessionsTitle}
              right={
                copy.sessionsBadge ? (
                  <PaperCardBadge>{copy.sessionsBadge}</PaperCardBadge>
                ) : undefined
              }
            />
            <PaperCardBody className="px-[18px] pb-[14px] pt-[6px]">
              <PaperThreadList
                rows={sessions}
                onSelect={onSelectSession}
                countLabel={copy.sessionPagesLabel}
                testId="paper-intelligence-sessions"
              />
            </PaperCardBody>
          </PaperCard>
        </div>

        <div className="flex flex-col gap-5">
          <PaperCard>
            <PaperCardHeader title={copy.threadsTitle} />
            <PaperCardBody className="px-[18px] pb-[14px] pt-[6px]">
              <PaperThreadList
                rows={threads}
                onSelect={onSelectThread}
                countLabel={copy.threadPagesLabel}
                testId="paper-intelligence-threads"
              />
            </PaperCardBody>
          </PaperCard>

          <PaperCard>
            <PaperCardHeader
              title={copy.refindTitle}
              right={
                copy.refindBadge ? (
                  <PaperCardBadge>{copy.refindBadge}</PaperCardBadge>
                ) : undefined
              }
            />
            <PaperCardBody className="px-[18px] pb-[14px] pt-[6px]">
              <PaperRefindShelf
                items={refindItems}
                resolveDomainColor={resolveDomainColor}
                resolveDomainAbbr={resolveDomainAbbr}
                onSelect={onSelectRefind}
                testId="paper-intelligence-refind"
              />
            </PaperCardBody>
          </PaperCard>
        </div>
      </div>
    </section>
  )
}
