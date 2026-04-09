import type { ReactNode } from 'react'

interface ErrorStateProps {
  title: string
  description: string
  eyebrow?: string
  action?: ReactNode
}

export function ErrorState({
  title,
  description,
  eyebrow = 'ATTENTION',
  action,
}: ErrorStateProps) {
  return (
    <section className="utility-block utility-block--danger" role="alert">
      <span className="mono-kicker">{eyebrow}</span>
      <h2>{title}</h2>
      <p>{description}</p>
      {action ? <div className="utility-block__actions">{action}</div> : null}
    </section>
  )
}
