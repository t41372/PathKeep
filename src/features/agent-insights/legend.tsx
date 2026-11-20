/** A chart's legend: a swatch and a name per series, wrapping as needed. */
import type { ReactNode } from 'react'

export function Legend({
  items,
}: {
  items: { label: ReactNode; color: string }[]
}) {
  return (
    <ul className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
      {items.map((item, index) => (
        <li key={index} className="flex items-center gap-1.5">
          <span
            aria-hidden
            className="size-2 rounded-[2px]"
            style={{ background: item.color }}
          />
          {item.label}
        </li>
      ))}
    </ul>
  )
}
