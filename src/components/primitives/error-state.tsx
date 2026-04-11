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
  eyebrow,
  action,
}: ErrorStateProps) {
  return (
    <section className="utility-block utility-block--danger" role="alert">
      {eyebrow ? <span className="mono-kicker">{eyebrow}</span> : null}
      <h2>{title}</h2>
      <p>{description}</p>
      {action ? <div className="utility-block__actions">{action}</div> : null}
    </section>
  )
}
