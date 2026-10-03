/** One row on a settings page: label and description on the left, control on the right. */
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export function SettingRow({
  title,
  description,
  control,
  children,
  danger,
  htmlFor,
}: {
  title: ReactNode
  description?: ReactNode
  control?: ReactNode
  /** Extra content under the row, e.g. progress or an inline form. */
  children?: ReactNode
  danger?: boolean
  htmlFor?: string
}) {
  return (
    <div className="flex flex-col gap-3 rounded-xl border bg-card px-4 py-3.5 shadow-card">
      <div className="flex items-center justify-between gap-4">
        <div className="flex min-w-0 flex-col gap-0.5">
          <label
            htmlFor={htmlFor}
            className={cn('font-medium', danger && 'text-destructive')}
          >
            {title}
          </label>
          {description && (
            <p className="text-[13px] text-muted-foreground">{description}</p>
          )}
        </div>
        {control && <div className="shrink-0">{control}</div>}
      </div>
      {children}
    </div>
  )
}

/** Heading + stack of rows, the body of one settings section. */
export function SettingsSection({
  title,
  children,
}: {
  title: ReactNode
  children: ReactNode
}) {
  return (
    <section className="flex max-w-[560px] flex-col gap-2.5">
      <h2 className="mb-2 text-xl font-semibold tracking-tight">{title}</h2>
      {children}
    </section>
  )
}
