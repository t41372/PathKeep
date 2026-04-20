import { Link } from 'react-router-dom'
import type { CompareSetPage } from '../../lib/core-intelligence'

function CompareSetPageRow({
  href,
  landingLabel,
  page,
  rowKey,
}: {
  href: string
  landingLabel: string
  page: CompareSetPage
  rowKey: string
}) {
  return (
    <div
      key={rowKey}
      className={`compare-set__page${page.isLanding ? ' compare-set__page--landing' : ''}`}
    >
      <Link className="compare-set__page-domain intelligence-link" to={href}>
        {page.registrableDomain}
      </Link>
      <span className="compare-set__page-title" title={page.title ?? page.url}>
        {page.title ?? page.url}
      </span>
      {page.isLanding ? (
        <span className="compare-set__landing-badge">{landingLabel}</span>
      ) : null}
    </div>
  )
}

export function CompareSetPageList({
  as = 'div',
  getHref,
  keyPrefix,
  landingLabel,
  maxItems,
  pages,
}: {
  as?: 'div' | 'ul'
  getHref: (page: CompareSetPage) => string
  keyPrefix: string
  landingLabel: string
  maxItems?: number
  pages: CompareSetPage[]
}) {
  const visiblePages = maxItems ? pages.slice(0, maxItems) : pages

  if (as === 'ul') {
    return (
      <ul className="compare-set__pages">
        {visiblePages.map((page, index) => (
          <li
            key={`${keyPrefix}:${page.canonicalUrl}:${index}`}
            className={`compare-set__page${page.isLanding ? ' compare-set__page--landing' : ''}`}
          >
            <Link
              className="compare-set__page-domain intelligence-link"
              to={getHref(page)}
            >
              {page.registrableDomain}
            </Link>
            <span
              className="compare-set__page-title"
              title={page.title ?? page.url}
            >
              {page.title ?? page.url}
            </span>
            {page.isLanding ? (
              <span className="compare-set__landing-badge">{landingLabel}</span>
            ) : null}
          </li>
        ))}
      </ul>
    )
  }

  return (
    <div className="compare-set__pages">
      {visiblePages.map((page, index) => (
        <CompareSetPageRow
          key={`${keyPrefix}:${page.canonicalUrl}:${index}`}
          href={getHref(page)}
          landingLabel={landingLabel}
          page={page}
          rowKey={`${keyPrefix}:${page.canonicalUrl}:${index}`}
        />
      ))}
    </div>
  )
}
