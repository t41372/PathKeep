/**
 * The title of the card an element is in, when it is plain text. A card's
 * controls use it when they speak about the card (Insights' "Ask about
 * this"). `SectionCard` provides it.
 */
import { createContext, useContext } from 'react'

export const CardTitle = createContext<string | null>(null)

export function useCardTitle() {
  return useContext(CardTitle)
}
