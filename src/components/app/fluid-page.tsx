/**
 * The page column of the Agent space (and of spaces after it): it starts
 * next to the rail and grows with the window instead of sitting in a fixed
 * centred column, so a wide window gets more columns, not wide margins
 * (`data-spaces-tradeoff.md` §4.5). Grids inside decide how many columns
 * fit; reading text keeps its own line length.
 */
import type { ReactNode } from 'react'
import { TitleBarStrip } from './title-bar'

export function FluidPage({ children }: { children: ReactNode }) {
  return (
    <div className="flex-1 overflow-y-auto">
      <TitleBarStrip />
      <div className="flex w-full flex-col gap-5 px-8 pt-9 pb-12 min-[1800px]:px-12">
        {children}
      </div>
    </div>
  )
}
