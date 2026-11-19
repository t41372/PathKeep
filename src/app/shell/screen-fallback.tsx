/**
 * What a screen shows while its code or its first answer loads: a title, a
 * row of stat cards and a large card, at their final sizes.
 */
import { Skeleton } from '@/components/ui/skeleton'

export function ScreenFallback() {
  return (
    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-5 px-10 py-9">
      <Skeleton className="h-9 w-64" />
      <div className="grid grid-cols-4 gap-3">
        {Array.from({ length: 4 }, (_, index) => (
          <Skeleton key={index} className="h-24 rounded-xl" />
        ))}
      </div>
      <Skeleton className="h-64 rounded-xl" />
    </div>
  )
}
