/**
 * Find in a conversation. Matches are counted from the data (so the count
 * covers events that aren't rendered yet); the visible ones are painted with
 * the CSS Custom Highlight API, which works inside rendered markdown without
 * touching its DOM.
 */
import { useEffect, useMemo } from 'react'
import type { TEvent } from '~/data/transcript'

export interface Match {
  event: number
  /** Which occurrence inside that event, in DOM text order. */
  nth: number
}

function eventText(e: TEvent): string {
  switch (e.k) {
    case 'user':
      return e.text
    case 'assistant':
      return e.md.replace(/[`*|#-]/g, ' ')
    case 'thinking':
      return e.text
    case 'tools':
      return e.calls.map((c) => `${c.arg} ${c.output ?? ''}`).join(' ')
    case 'subagent':
      return e.sub.title
    default:
      return ''
  }
}

export function useMatches(events: TEvent[], query: string): Match[] {
  return useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return []
    const out: Match[] = []
    events.forEach((e, i) => {
      const text = eventText(e).toLowerCase()
      let from = 0
      let nth = 0
      for (;;) {
        const at = text.indexOf(q, from)
        if (at < 0) break
        out.push({ event: i, nth: nth++ })
        from = at + q.length
      }
    })
    return out
  }, [events, query])
}

type HighlightRegistry = { set: (name: string, h: unknown) => void; delete: (name: string) => void }
declare const Highlight: new (...ranges: Range[]) => unknown

function registry(): HighlightRegistry | null {
  const css = (globalThis as unknown as { CSS?: { highlights?: HighlightRegistry } }).CSS
  return css?.highlights ?? null
}

/** Paints matches inside `root` and marks the current one. Re-run on scroll. */
export function paintMatches(root: HTMLElement | null, query: string, current: Match | undefined) {
  const reg = registry()
  if (!reg) return
  reg.delete('find')
  reg.delete('find-current')
  const q = query.trim().toLowerCase()
  if (!root || !q) return
  const all: Range[] = []
  let currentRange: Range | null = null
  for (const block of root.querySelectorAll<HTMLElement>('[data-event]')) {
    const index = Number(block.dataset.event)
    let nth = 0
    const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT)
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = (node.textContent ?? '').toLowerCase()
      let from = 0
      for (;;) {
        const at = text.indexOf(q, from)
        if (at < 0) break
        const range = document.createRange()
        range.setStart(node, at)
        range.setEnd(node, at + q.length)
        if (current && current.event === index && current.nth === nth) currentRange = range
        else all.push(range)
        nth++
        from = at + q.length
      }
    }
  }
  reg.set('find', new Highlight(...all))
  if (currentRange) reg.set('find-current', new Highlight(currentRange))
}

export function useClearHighlights() {
  useEffect(
    () => () => {
      const reg = registry()
      reg?.delete('find')
      reg?.delete('find-current')
    },
    [],
  )
}
