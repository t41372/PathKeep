/** The prototype's card: title, optional subtitle and action, then content. */
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export function SectionCard({
  title,
  subtitle,
  action,
  children,
  className,
}: {
  title?: ReactNode
  subtitle?: ReactNode
  action?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <section
      className={cn(
        'flex min-w-0 flex-col gap-3.5 rounded-xl border bg-card p-5 shadow-card',
        className,
      )}
    >
      {(title || action) && (
        <header className="flex items-start justify-between gap-2">
          <div className="flex flex-col gap-1">
            {title && <h2 className="text-sm font-semibold">{title}</h2>}
            {subtitle && (
              <p className="text-[13px] text-muted-foreground">{subtitle}</p>
            )}
          </div>
          {action}
        </header>
      )}
      {children}
    </section>
  )
}

/** Page header used by Home, Insights and Backup. */
export function PageHeader({
  title,
  eyebrow,
  subtitle,
  actions,
}: {
  title: ReactNode
  eyebrow?: ReactNode
  subtitle?: ReactNode
  actions?: ReactNode
}) {
  return (
    <header className="flex flex-wrap items-end justify-between gap-4">
      <div className="flex flex-col gap-1">
        {eyebrow && (
          <span className="text-[13px] text-muted-foreground">{eyebrow}</span>
        )}
        <h1 className="text-[30px] leading-tight font-semibold tracking-[-0.02em]">
          {title}
        </h1>
        {subtitle && (
          <p className="text-[13px] text-muted-foreground">{subtitle}</p>
        )}
      </div>
      {actions && <div className="flex items-center gap-2.5">{actions}</div>}
    </header>
  )
}

/** Scrollable page column with the prototype's width and padding. */
export function PageScroll({
  children,
  wide,
}: {
  children: ReactNode
  wide?: boolean
}) {
  return (
    <div className="flex-1 overflow-y-auto">
      <div
        className={cn(
          'mx-auto flex flex-col gap-5 px-10 pt-9 pb-12',
          wide ? 'max-w-[1080px]' : 'max-w-[1040px]',
        )}
      >
        {children}
      </div>
    </div>
  )
}
