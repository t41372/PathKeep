/**
 * Rows of a Projects card, virtualized against the page's own scroll
 * (`agent-history.md` §5.1: rows of about 40 px follow the page), so a
 * card of thousands of projects draws only what is on screen and the page
 * still scrolls as one.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import {
  useContext,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { PageScrollContext, ROW_HEIGHT } from './projects-context'

export function PageRows<T>({
  items,
  keyOf,
  children,
}: {
  items: T[]
  keyOf: (item: T) => string
  children: (item: T, index: number) => ReactNode
}) {
  const parent = useContext(PageScrollContext)
  const list = useRef<HTMLDivElement>(null)
  const [margin, setMargin] = useState(0)
  useLayoutEffect(() => {
    const element = list.current
    if (!element || !parent) return
    const measure = () => {
      const top =
        element.getBoundingClientRect().top -
        parent.getBoundingClientRect().top +
        parent.scrollTop
      setMargin((previous) => (Math.abs(previous - top) < 1 ? previous : top))
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(parent)
    if (parent.firstElementChild) observer.observe(parent.firstElementChild)
    return () => observer.disconnect()
  }, [parent])
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parent,
    estimateSize: () => ROW_HEIGHT,
    getItemKey: (index) => keyOf(items[index]),
    scrollMargin: margin,
    overscan: 10,
  })
  return (
    <div
      ref={list}
      role="list"
      className="relative w-full"
      style={{ height: virtualizer.getTotalSize() }}
    >
      {virtualizer.getVirtualItems().map((item) => (
        <div
          key={item.key}
          role="listitem"
          className="absolute top-0 left-0 w-full"
          style={{
            height: ROW_HEIGHT,
            transform: `translateY(${item.start - margin}px)`,
          }}
        >
          {children(items[item.index], item.index)}
        </div>
      ))}
    </div>
  )
}
