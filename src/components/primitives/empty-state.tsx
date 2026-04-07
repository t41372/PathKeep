import type { ReactNode } from 'react'

interface EmptyStateProps {
  eyebrow: string
  title: string
  description: string
  action?: ReactNode
}

export function EmptyState({
  eyebrow,
  title,
  description,
  action,
}: EmptyStateProps) {
  return (
    <section className="utility-block" data-testid="empty-state">
      <span className="mono-kicker">{eyebrow}</span>
      <h2>{title}</h2>
      <p>{description}</p>
      {action ? <div className="utility-block__actions">{action}</div> : null}
    </section>
  )
}
