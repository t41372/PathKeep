/**
 * Dashboard "What you've been thinking about" active threads card.
 *
 * The real backend mapping to query_families / path_flows happens during the
 * Intelligence route sweep. Until that wiring lands, this card renders an
 * honest empty state that points the user at /intelligence rather than
 * displaying invented "Tokio scheduler deep dive"-style sample threads as
 * though they were real archive insights. Trust & Transparency requires that
 * surfaces in `ready` state never carry fabricated user-data lookalikes.
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { useI18n } from '@/lib/i18n'

export interface DashboardActiveThreadsProps {
  onOpenAll: () => void
  /** Reserved for the future intelligence wiring; called by the empty-state CTA. */
  onOpenThread?: (threadId: string) => void
}

export function DashboardActiveThreads({
  onOpenAll,
}: DashboardActiveThreadsProps) {
  const { t } = useI18n()
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
        <p
          className="m-0 font-serif text-[13.5px] italic leading-[1.55] text-ink-muted"
          data-testid="dashboard-active-threads-empty"
        >
          {t('dashboard.activeThreadsEmpty')}
        </p>
      </PaperCardBody>
    </PaperCard>
  )
}
