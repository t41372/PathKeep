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
    <div className="flex flex-col gap-3 rounded-xl border bg-card px-[18px] py-4 shadow-card">
      <div className="flex items-center justify-between gap-5">
        <div className="flex min-w-0 flex-col gap-[3px]">
          <label
            htmlFor={htmlFor}
            className={cn('font-medium', danger && 'text-destructive')}
          >
            {title}
          </label>
          {description && (
            <div className="text-[13px] leading-[1.45] [overflow-wrap:anywhere] text-muted-foreground">
              {description}
            </div>
          )}
        </div>
        {control && (
          <div className="flex shrink-0 items-center gap-2">{control}</div>
        )}
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
    <section className="flex flex-col gap-3">
      <h2 className="mb-2 text-xl font-semibold tracking-tight">{title}</h2>
      {children}
    </section>
  )
}

/** A quiet label that splits a long section into groups. */
export function SettingsGroup({
  title,
  note,
}: {
  title: ReactNode
  note?: ReactNode
}) {
  return (
    <div className="mt-3 flex flex-col gap-0.5 px-1">
      <h3 className="text-[13px] font-medium text-muted-foreground">{title}</h3>
      {note && <p className="text-xs text-muted-foreground">{note}</p>}
    </div>
  )
}
