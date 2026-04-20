/**
 * Shared intelligence-entity action links.
 *
 * Why this file exists:
 * - M7 promotes multiple entities to route-first destinations, so link/button
 *   grammar should not be rebuilt independently by every consumer.
 * - Explorer, route pages, and Settings external-output review all need the
 *   same entity-first CTA styling with only small variant changes.
 */

import { Link, useInRouterContext } from 'react-router-dom'

export interface InsightEntityActionLink {
  href: string
  key?: string
  label: string
  style?: 'button' | 'chip' | 'text'
}

function actionLinkClassName(style?: InsightEntityActionLink['style']) {
  return style === 'chip'
    ? 'chip-button'
    : style === 'text'
      ? 'intelligence-link'
      : 'btn-secondary'
}

function isInternalRouteHref(href: string) {
  return href.startsWith('/')
}

export function InsightEntityActions({
  className = 'intelligence-actions',
  items,
}: {
  className?: string
  items: InsightEntityActionLink[]
}) {
  const inRouterContext = useInRouterContext()

  return (
    <div className={className}>
      {items.map((item) => {
        const key =
          item.key ?? `${item.style ?? 'button'}:${item.href}:${item.label}`
        const className = actionLinkClassName(item.style)

        // Internal destinations must flow through React Router so HashRouter can
        // emit `#/...` links for the desktop shell instead of naked paths.
        if (inRouterContext && isInternalRouteHref(item.href)) {
          return (
            <Link key={key} className={className} to={item.href}>
              {item.label}
            </Link>
          )
        }

        return (
          <a key={key} className={className} href={item.href}>
            {item.label}
          </a>
        )
      })}
    </div>
  )
}
