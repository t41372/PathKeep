/**
 * @file active-threads-card.tsx
 * @description Dashboard "What you've been thinking about" card. Wired to the real PathFlows backend so the dashboard surfaces meaningful repeated browsing sequences instead of fabricated samples.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Fetch the top recurring 3-step path flows for the last 30 days within the
 *   active profile scope.
 * - Render up to three flows as arrow-chain rows with occurrence counts.
 * - Surface honest loading / empty / error states without inventing sample
 *   threads (Trust & Transparency).
 *
 * ## Not responsible for
 * - Deep-link routing into Intelligence focus state — the parent route owns
 *   `onOpenThread` and `onOpenAll`. The card hands the parent the whole flow
 *   (not just its id) because choosing the deep-link target needs the flow's
 *   step domains, and that route grammar must not leak into this render shell.
 * - Query family rendering (the secondary intelligence-sections render that;
 *   the dashboard scope stays pinned to recurring path flows so it does not
 *   duplicate the Intelligence route's search-activity surface).
 *
 * ## Dependencies
 * - `coreIntelligenceApi.getPathFlows` for the data fetch.
 * - `useProfileScope` for the active profile.
 *
 * ## Performance notes
 * - Only fetches when the archive is initialized + unlocked; otherwise stays
 *   idle so the card never fires backend queries before the first run.
 * - Caps at three rendered rows even though the API returns up to ten — this
 *   keeps the dashboard card visually balanced against the archive sibling.
 */

import { useEffect, useState } from 'react'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { Skeleton } from '@/components/primitives/skeleton'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import * as coreIntelligenceApi from '@/lib/core-intelligence/api'
import type { PathFlow } from '@/lib/core-intelligence'
import { useProfileScope } from '@/lib/profile-scope-context'
import { dashboardThreadsRange } from './dashboard-helpers'

export interface DashboardActiveThreadsProps {
  onOpenAll: () => void
  /**
   * Optional handler invoked with the full flow when the user clicks a thread
   * row. The parent needs the flow's steps (not just its id) to build a
   * deep-link that actually surfaces the flow in the Intelligence route.
   */
  onOpenThread?: (flow: PathFlow) => void
  /** Whether the archive is ready to be queried (initialized + unlocked). */
  archiveReady: boolean
}

const MAX_RENDERED_FLOWS = 3

export function DashboardActiveThreads({
  onOpenAll,
  onOpenThread,
  archiveReady,
}: DashboardActiveThreadsProps) {
  const { t } = useI18n()
  const { activeProfileId } = useProfileScope()

  const [flows, setFlows] = useState<PathFlow[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!archiveReady) {
      setFlows([])
      setLoading(false)
      setError(null)
      return
    }
    let cancelled = false
    setLoading(true)
    void (async () => {
      try {
        const range = dashboardThreadsRange(new Date())
        const result = await coreIntelligenceApi.getPathFlows(
          range,
          activeProfileId,
          3,
          10,
        )
        if (!cancelled) {
          setFlows(result.data ?? [])
          setError(null)
        }
      } catch (nextError) {
        if (!cancelled) {
          setFlows([])
          setError(describeError(nextError, 'get_path_flows'))
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [activeProfileId, archiveReady, t])

  const visibleFlows = flows.slice(0, MAX_RENDERED_FLOWS)

  return (
    <PaperCard testId="dashboard-active-threads">
      <PaperCardHeader
        title={t('dashboard.activeThreadsTitle')}
        right={
          <PaperCardBadge onClick={onOpenAll}>
            {t('dashboard.activeThreadsAll')} →
          </PaperCardBadge>
        }
      />
      <PaperCardBody className="px-[18px] pt-1 pb-[14px]">
        {loading ? (
          <ul
            className="m-0 flex list-none flex-col gap-[10px] p-0"
            data-testid="dashboard-active-threads-loading"
            aria-busy="true"
            aria-label={t('common.loading')}
          >
            {[0, 1, 2].map((index) => (
              <li key={index}>
                <Skeleton className="h-[56px] w-full" />
              </li>
            ))}
          </ul>
        ) : error ? (
          <p
            className="m-0 font-serif text-[13.5px] italic leading-[1.55] text-danger"
            data-testid="dashboard-active-threads-error"
          >
            {error}
          </p>
        ) : visibleFlows.length === 0 ? (
          <p
            className="m-0 font-serif text-[13.5px] italic leading-[1.55] text-ink-muted"
            data-testid="dashboard-active-threads-empty"
          >
            {t('dashboard.activeThreadsEmpty')}
          </p>
        ) : (
          <ul
            className="m-0 flex list-none flex-col gap-[10px] p-0"
            data-testid="dashboard-active-threads-list"
          >
            {visibleFlows.map((flow) => (
              <ThreadRow
                key={flow.flowId}
                flow={flow}
                onOpenThread={onOpenThread}
                occurrenceLabel={t('dashboard.pathFlowsOccurrences', {
                  count: flow.occurrenceCount,
                })}
              />
            ))}
          </ul>
        )}
      </PaperCardBody>
    </PaperCard>
  )
}

interface ThreadRowProps {
  flow: PathFlow
  occurrenceLabel: string
  onOpenThread?: (flow: PathFlow) => void
}

function ThreadRow({ flow, occurrenceLabel, onOpenThread }: ThreadRowProps) {
  const handleClick = () => {
    if (onOpenThread) onOpenThread(flow)
  }
  return (
    <li>
      <button
        type="button"
        onClick={handleClick}
        data-testid={`dashboard-active-threads-row-${flow.flowId}`}
        className="border-border-light hover:bg-hover w-full rounded-paper border bg-paper px-3 py-[10px] text-left transition-colors duration-150"
      >
        <div className="flex flex-wrap items-baseline gap-[6px] font-mono text-[11.5px] text-ink">
          {flow.steps.map((step, index) => (
            <span
              key={`${flow.flowId}:${step.index}`}
              className="inline-flex items-baseline gap-[6px]"
            >
              <span className="rounded-[2px] bg-accent-soft px-[5px] py-[1px] text-accent-text">
                {step.label}
              </span>
              {index < flow.steps.length - 1 ? (
                <span className="text-ink-faint" aria-hidden="true">
                  →
                </span>
              ) : null}
            </span>
          ))}
        </div>
        <div className="mt-[5px] font-mono text-[9.5px] uppercase tracking-[0.07em] text-ink-faint">
          {occurrenceLabel}
        </div>
      </button>
    </li>
  )
}
