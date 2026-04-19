/**
 * Shared intelligence-entity action links.
 *
 * Why this file exists:
 * - M7 promotes multiple entities to route-first destinations, so link/button
 *   grammar should not be rebuilt independently by every consumer.
 * - Explorer, route pages, and Settings external-output review all need the
 *   same entity-first CTA styling with only small variant changes.
 */

export interface InsightEntityActionLink {
  href: string
  key?: string
  label: string
  style?: 'button' | 'chip' | 'text'
}

export function InsightEntityActions({
  className = 'intelligence-actions',
  items,
}: {
  className?: string
  items: InsightEntityActionLink[]
}) {
  return (
    <div className={className}>
      {items.map((item) => (
        <a
          key={
            item.key ?? `${item.style ?? 'button'}:${item.href}:${item.label}`
          }
          className={
            item.style === 'chip'
              ? 'chip-button'
              : item.style === 'text'
                ? 'intelligence-link'
                : 'btn-secondary'
          }
          href={item.href}
        >
          {item.label}
        </a>
      ))}
    </div>
  )
}
