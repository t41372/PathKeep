/** A card whose section has not arrived yet: its frame, quietly shimmering. */
import { Skeleton } from '@/components/ui/skeleton'

export function PendingCard() {
  return (
    <div
      aria-hidden
      className="flex min-w-0 flex-col gap-3.5 rounded-xl border bg-card p-5 shadow-card"
    >
      <Skeleton className="h-4 w-40" />
      <Skeleton className="h-3 w-64 max-w-full" />
      <Skeleton className="h-[160px]" />
    </div>
  )
}
