import * as React from 'react'
import { cn } from '@/lib/cn'
import { Progress as ProgressPrimitive } from 'radix-ui'

function Progress({
  className,
  value,
  ...props
}: React.ComponentProps<typeof ProgressPrimitive.Root>) {
  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      className={cn(
        'relative h-2 w-full overflow-hidden rounded-full bg-primary/20',
        className,
      )}
      {...props}
    >
      {value == null ? (
        // No number yet: a sliding segment says "working" instead of an
        // empty bar that reads as "nothing happening".
        <ProgressPrimitive.Indicator
          data-slot="progress-indicator"
          className="h-full w-1/3 animate-indeterminate rounded-full bg-primary"
        />
      ) : (
        <ProgressPrimitive.Indicator
          data-slot="progress-indicator"
          className="h-full w-full flex-1 bg-primary transition-all"
          style={{ transform: `translateX(-${100 - value}%)` }}
        />
      )}
    </ProgressPrimitive.Root>
  )
}

export { Progress }
