import { useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import type { QueryFamily } from '../../lib/core-intelligence'
import { formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n/hooks'

type QueryFamilyLinkMode = 'none' | 'anchor' | 'card'

export function QueryFamilyCard({
  family,
  footer,
  href,
  linkMode = 'anchor',
  memberCountLabel,
  moreLabel,
  showAnchor = true,
  showDates = true,
  showMembers = true,
}: {
  family: QueryFamily
  footer?: ReactNode
  href?: string
  linkMode?: QueryFamilyLinkMode
  memberCountLabel: string
  moreLabel?: (hiddenCount: number) => string
  showAnchor?: boolean
  showDates?: boolean
  showMembers?: boolean
}) {
  const { language } = useI18n()
  const canExpand = showMembers && typeof moreLabel === 'function'
  const [expanded, setExpanded] = useState(!canExpand)
  const visibleQueries =
    showMembers && (!canExpand || expanded)
      ? family.queries
      : family.queries.slice(0, 3)

  const anchor =
    href && linkMode !== 'none' ? (
      <Link className="query-family-card__anchor intelligence-link" to={href}>
        "{family.anchorQuery}"
      </Link>
    ) : (
      <span className="query-family-card__anchor">"{family.anchorQuery}"</span>
    )

  const firstSeenLabel =
    (language ? formatDateTime(family.firstSeenAt, language) : null) ??
    family.firstSeenAt
  const lastSeenLabel =
    (language ? formatDateTime(family.lastSeenAt, language) : null) ??
    family.lastSeenAt

  const body = (
    <>
      <div className="query-family-card__header">
        {showAnchor ? (
          linkMode === 'card' ? (
            <span className="query-family-card__anchor">
              "{family.anchorQuery}"
            </span>
          ) : (
            anchor
          )
        ) : null}
        <span className="query-family-card__engine">{family.searchEngine}</span>
        <span className="query-family-card__count">
          {family.memberCount} {memberCountLabel}
        </span>
      </div>
      {showMembers ? (
        <div className="query-family-card__members">
          {visibleQueries.map((query, index) => (
            <span
              key={`${family.familyId}:${query}:${index}`}
              className="query-family-card__member"
            >
              "{query}"
            </span>
          ))}
          {canExpand && family.queries.length > 3 && !expanded ? (
            <button
              className="intelligence-link"
              type="button"
              onClick={(event) => {
                event.preventDefault()
                event.stopPropagation()
                setExpanded(true)
              }}
            >
              {moreLabel(family.queries.length - 3)}
            </button>
          ) : null}
        </div>
      ) : null}
      {showDates ? (
        <span className="query-family-card__dates">
          {firstSeenLabel} - {lastSeenLabel}
        </span>
      ) : null}
      {footer}
    </>
  )

  if (href && linkMode === 'card') {
    return (
      <Link className="query-family-card" to={href}>
        {body}
      </Link>
    )
  }

  return <div className="query-family-card">{body}</div>
}
