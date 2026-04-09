import type { ReactNode } from 'react'

interface StatusCalloutProps {
  tone: 'info' | 'warning' | 'danger' | 'blocked' | 'success'
  title: string
  body?: string
  eyebrow?: string
  actions?: ReactNode
}

export function StatusCallout({
  tone,
  title,
  body,
  eyebrow,
  actions,
}: StatusCalloutProps) {
  return (
    <section className={`status-callout status-callout--${tone}`}>
      {eyebrow ? <p className="mono-kicker">{eyebrow}</p> : null}
      <h3>{title}</h3>
      {body ? <p>{body}</p> : null}
      {actions ? <div className="utility-block__actions">{actions}</div> : null}
    </section>
  )
}
