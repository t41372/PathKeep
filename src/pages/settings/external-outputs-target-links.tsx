/**
 * @file external-outputs-target-links.tsx
 * @description Renders the shared review-link row for external-output cards without leaking route grammar into every tab.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Turn one embed/widget card's insight references into review links.
 * - Keep fallback href display and secondary-target labeling consistent across tabs.
 *
 * ## Not responsible for
 * - Fetching output payloads
 * - Rendering the surrounding card or review section UI
 *
 * ## Dependencies
 * - Depends on review primitives, i18n hooks, and shared intelligence href helpers.
 *
 * ## Performance notes
 * - Builds a small link list per rendered card; keep it simple because embed/widget previews can render several cards at once.
 */

import {
  ReviewTargetLinksRow,
  type ReviewTargetLink,
} from '../../components/review'
import type {
  DateRange,
  IntelligenceEmbedCardPayload,
} from '../../lib/core-intelligence'
import { useI18n } from '../../lib/i18n'
import {
  insightEntityReferenceHref,
  insightEntityReferenceLabel,
} from '../../lib/intelligence'
import type { Translate } from './external-outputs-shared'

interface OutputTargetLinksProps {
  activeProfileId: string | null
  card: IntelligenceEmbedCardPayload
  dateRange: DateRange
  t: Translate
}

/**
 * Centralizes how external-output cards point back into first-party intelligence routes.
 */
export function OutputTargetLinks({
  activeProfileId,
  card,
  dateRange,
  t,
}: OutputTargetLinksProps) {
  const { ns } = useI18n()
  const intelligenceT = ns('intelligence')
  const primaryHref = card.primaryTarget
    ? insightEntityReferenceHref(card.primaryTarget, {
        dateRange,
        preset: 'custom',
        profileId: activeProfileId,
      })
    : null
  const secondaryTargets = card.secondaryTargets ?? []

  if (!primaryHref && secondaryTargets.length === 0 && !card.href) {
    return null
  }

  return (
    <ReviewTargetLinksRow
      fallback={card.href ? <span className="mono">{card.href}</span> : null}
      label={t('externalOutputsHref')}
      primaryHref={primaryHref}
      primaryLabel={t('externalOutputsOpenInsights')}
      secondaryLinks={secondaryTargets.map<ReviewTargetLink>(
        (target, index) => ({
          href: insightEntityReferenceHref(target, {
            dateRange,
            preset: 'custom',
            profileId: activeProfileId,
          }),
          key: `${card.cardId}:${target.kind}:${index}`,
          label: insightEntityReferenceLabel(target, intelligenceT),
        }),
      )}
    />
  )
}
