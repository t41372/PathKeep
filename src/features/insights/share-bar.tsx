/**
 * How a row compares with the largest in its list: a short track beside the
 * row's number. It replaces bars drawn behind the whole row, which at small
 * shares shrank to a blob behind the icon.
 */
import { cn } from '@/lib/cn'

const tones = {
  brand: 'bg-brand/70',
  blue: 'bg-blue/70',
  muted: 'bg-muted-foreground/40',
} as const

export function ShareBar({
  value,
  max,
  tone = 'brand',
}: {
  value: number
  max: number
  tone?: keyof typeof tones
}) {
  return (
    <span
      aria-hidden
      className="h-1.5 w-16 shrink-0 overflow-hidden rounded-full bg-muted"
    >
      <span
        className={cn(
          'block h-full rounded-full transition-[width] duration-500',
          tones[tone],
        )}
        style={{ width: `${(Math.max(0, value) / Math.max(1, max)) * 100}%` }}
      />
    </span>
  )
}
