/**
 * Newspaper-style hub layout for the Intelligence overview page.
 *
 * ## Responsibilities
 * - Provide the Paper-design layout primitives used by the Intelligence hub:
 *   `AxisCard` (a three-axis preview card) and `SpotlightCard` (the
 *   above-the-fold highlight).
 * - Wrap each axis card in PaperCard components for the Paper design language.
 *
 * ## Not responsible for
 * - Fetching data (parent coordinator owns data loading and cache peeks).
 * - Owning individual section internals (delegated to sibling section modules).
 * - Styling beyond layout composition (CSS lives in page-shell.css / hub.css).
 * - Gating secondary-section render on scroll (owned by `LazySection`).
 *
 * ## Dependencies
 * - PaperCard primitives from `@/components/cards`.
 */

import { type ReactNode } from 'react'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'

// ---------------------------------------------------------------------------
// Layer 2: Axis preview card
// ---------------------------------------------------------------------------

interface AxisCardProps {
  /** Card title shown in the PaperCardHeader */
  title: string
  /** "See all" CTA label */
  seeAllLabel: string
  /** Callback when "See all" is clicked — scrolls to the full section */
  onSeeAll?: () => void
  /** Preview items rendered inside the card body */
  children: ReactNode
  /** Test id forwarded to PaperCard */
  testId: string
  /** Optional className on the outer wrapper */
  className?: string
}

/**
 * A PaperCard that shows 2-3 preview items from one navigation axis
 * plus a "See all" CTA in the header.
 */
export function AxisCard({
  title,
  seeAllLabel,
  onSeeAll,
  children,
  testId,
  className,
}: AxisCardProps) {
  return (
    <PaperCard testId={testId} className={className}>
      <PaperCardHeader
        title={title}
        compact
        right={
          onSeeAll ? (
            <PaperCardBadge onClick={onSeeAll}>
              {seeAllLabel} &rarr;
            </PaperCardBadge>
          ) : undefined
        }
      />
      <PaperCardBody className="px-[18px] py-[14px]">{children}</PaperCardBody>
    </PaperCard>
  )
}

// ---------------------------------------------------------------------------
// Spotlight card
// ---------------------------------------------------------------------------

interface SpotlightCardProps {
  /** Primary text — one sentence describing the period highlight */
  sentence: string | null
}

/**
 * Single accent-bordered card that surfaces the most notable insight
 * above the fold.
 */
export function SpotlightCard({ sentence }: SpotlightCardProps) {
  if (!sentence) {
    return null
  }

  return (
    <PaperCard accent testId="hub-spotlight">
      <PaperCardBody className="px-[18px] py-[14px]">
        <p className="font-serif text-[14px] leading-relaxed text-ink m-0">
          {sentence}
        </p>
      </PaperCardBody>
    </PaperCard>
  )
}
