/**
 * A list that scrolls inside its card instead of making the card taller.
 * The list's own edge fades out where there is more above or below.
 *
 * The fade is a mask on the list (its content goes transparent, showing
 * whatever is behind), not a layer painted over it: a gradient in the card's
 * colour turned into a light band on a translucent card in dark mode, and
 * covered most of a row (`WORK-UI-POLISH-2026-10`).
 *
 * With `fill`, the list takes the height the card has left, at least
 * `maxHeight`: a card beside a taller one shows more rows instead of an empty
 * space, and the list never makes the row taller than its neighbour does.
 */
import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { cn } from '@/lib/cn'

/** How far the fade reaches into the list. */
const FADE = '20px'

export function ScrollList({
  maxHeight,
  fill = false,
  children,
}: {
  maxHeight: number
  fill?: boolean
  children: ReactNode
}) {
  const box = useRef<HTMLDivElement>(null)
  const [edges, setEdges] = useState({ above: false, below: false })
  const measure = useCallback(() => {
    const element = box.current
    if (!element) return
    const above = element.scrollTop > 1
    const below =
      element.scrollTop + element.clientHeight < element.scrollHeight - 1
    setEdges((old) =>
      old.above === above && old.below === below ? old : { above, below },
    )
  }, [])
  useLayoutEffect(() => measure(), [measure, children])
  useLayoutEffect(() => {
    const element = box.current
    if (!fill || !element) return
    const observer = new ResizeObserver(measure)
    observer.observe(element)
    return () => observer.disconnect()
  }, [fill, measure])
  const top = edges.above ? FADE : '0px'
  const bottom = edges.below ? FADE : '0px'
  const mask =
    edges.above || edges.below
      ? `linear-gradient(to bottom, transparent, #000 ${top}, #000 calc(100% - ${bottom}), transparent)`
      : undefined
  return (
    <div
      className={cn('relative', fill && 'min-h-0 flex-1')}
      style={fill ? { minHeight: maxHeight } : undefined}
    >
      <div
        ref={box}
        onScroll={measure}
        className={cn(
          'overflow-y-auto overscroll-contain',
          fill && 'absolute inset-0',
        )}
        style={{
          ...(fill ? {} : { maxHeight }),
          maskImage: mask,
          WebkitMaskImage: mask,
        }}
      >
        {children}
      </div>
    </div>
  )
}
