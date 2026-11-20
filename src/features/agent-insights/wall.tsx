/**
 * The wall of cards (`agent-history.md` §2.4): as many columns as fit,
 * each card as tall as its content, and no gap under a short card. The
 * order is the order of importance: cards are placed in reading order,
 * never back-filled, so a card's top edge is never above an earlier
 * card's, whatever the width.
 *
 * Each brick measures its card and spans that many 8 px grid rows; the
 * grid places them in order. 456 px is the narrowest column the weekly
 * rhythm fits in, and still gives two columns on a laptop.
 */
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'

const ROW = 8
const GAP = 16

export function Wall({ children }: { children: ReactNode }) {
  return (
    <div className="grid auto-rows-[8px] grid-cols-[repeat(auto-fill,minmax(min(100%,456px),1fr))] gap-x-4">
      {children}
    </div>
  )
}

export function Brick({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null)
  const [span, setSpan] = useState(40)
  useLayoutEffect(() => {
    const card = ref.current?.firstElementChild as HTMLElement | null
    if (!card) return
    const measure = () =>
      setSpan(Math.ceil((card.getBoundingClientRect().height + GAP) / ROW))
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(card)
    return () => observer.disconnect()
  }, [])
  return (
    <div ref={ref} style={{ gridRowEnd: `span ${span}` }}>
      <div className="flex flex-col">{children}</div>
    </div>
  )
}
