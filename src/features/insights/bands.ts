/**
 * Whether each of Insights' two batches (see `overview.ts`) has settled, for
 * the cards that wait for theirs. Outside the Insights page, and for cards in
 * no batch, every card reads by itself straight away.
 */
import { createContext, useContext } from 'react'

export type Band = 'primary' | 'secondary'

const BandsContext = createContext<Record<Band, boolean>>({
  primary: true,
  secondary: true,
})
export const BandsProvider = BandsContext.Provider

export function useBandSettled(band: Band | null) {
  const bands = useContext(BandsContext)
  return band === null || bands[band]
}
