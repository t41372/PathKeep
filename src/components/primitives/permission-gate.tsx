import type { ReactNode } from 'react'

interface PermissionGateProps {
  eyebrow: string
  title: string
  detail: string
  children?: ReactNode
}

export function PermissionGate({
  eyebrow,
  title,
  detail,
  children,
}: PermissionGateProps) {
  return (
    <section className="shell-panel permission-gate">
      <div className="panel-header">
        <span className="panel-title">{eyebrow}</span>
      </div>
      <div className="panel-body">
        <h2>{title}</h2>
        <p>{detail}</p>
        {children ? (
          <div className="utility-block__actions">{children}</div>
        ) : null}
      </div>
    </section>
  )
}
