/**
 * Agent pages use the whole panel: content starts next to the rail and
 * grows with the window instead of sitting in a fixed centred column with
 * wide empty margins. Grids inside decide how many columns fit.
 */
import type { ReactNode } from 'react'

export function FluidPage({ children }: { children: ReactNode }) {
  return (
    <div className="flex-1 overflow-y-auto">
      <div className="flex w-full flex-col gap-5 px-8 pt-8 pb-12 min-[1800px]:px-12">{children}</div>
    </div>
  )
}
