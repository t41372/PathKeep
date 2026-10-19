/**
 * The prototype's bordered option cards (encryption, schedule, AI), built on
 * a radio group so arrow keys and screen readers treat them as one choice.
 */
import type { ReactNode } from 'react'
import { RadioGroup } from 'radix-ui'
import { cn } from '@/lib/cn'

export function ChoiceGroup<T extends string>({
  value,
  onValueChange,
  label,
  className,
  children,
}: {
  value: T
  onValueChange: (value: T) => void
  label: string
  className?: string
  children: ReactNode
}) {
  return (
    <RadioGroup.Root
      value={value}
      onValueChange={(next) => onValueChange(next as T)}
      aria-label={label}
      className={cn('flex flex-col gap-2', className)}
    >
      {children}
    </RadioGroup.Root>
  )
}

export function ChoiceCard({
  value,
  title,
  body,
  leading,
  trailing,
  stacked,
}: {
  value: string
  title: ReactNode
  body: ReactNode
  leading?: ReactNode
  trailing?: ReactNode
  /** Icon above the text instead of beside it. */
  stacked?: boolean
}) {
  return (
    <RadioGroup.Item
      value={value}
      className={cn(
        'group flex gap-3.5 rounded-xl border-[1.5px] bg-card p-4 text-left shadow-card transition-[border-color,box-shadow] duration-150 outline-none hover:border-foreground/30 focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=checked]:border-foreground',
        stacked ? 'flex-col gap-1.5' : 'items-start',
      )}
    >
      {leading}
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-medium">{title}</span>
        <span className="text-xs leading-relaxed text-muted-foreground">
          {body}
        </span>
      </span>
      {trailing}
    </RadioGroup.Item>
  )
}

/** The round radio mark used by the schedule options. */
export function RadioMark() {
  return (
    <span className="mt-0.5 flex size-[18px] shrink-0 items-center justify-center rounded-full border-[1.5px] border-border transition-colors group-data-[state=checked]:border-foreground">
      <span className="size-2 scale-0 rounded-full bg-foreground transition-transform duration-150 group-data-[state=checked]:scale-100" />
    </span>
  )
}
