/**
 * Lights find's occurrences where they are drawn (`agent-history.md` §4.6):
 * every one on screen, and the current one stronger. Rows are virtualized
 * and parts fold, so this works on what is in the page right now and runs
 * again whenever the page changes while find is open.
 *
 * Each drawn part says which record parts it shows (`data-part`, tokens
 * from `partToken`). Inside it, the matched words are found again in the
 * drawn text, case-insensitively, with any run of spaces matching any
 * other: the backend's offsets are in the readable text, which the drawn
 * Markdown or code does not keep character for character. The current
 * occurrence is the k-th time its words appear in its part, k counted
 * among the occurrences before it there.
 *
 * Painting uses the CSS Custom Highlight API, so no row is re-rendered.
 * Where a webview does not have it, nothing is lit; the count and moving
 * between occurrences still work.
 */
import type { SearchMatch } from '@/lib/backend-client/agent-search'
import { matchedText, partToken, type FoundMatches } from './use-find'

const ALL = 'pathkeep-find'
const CURRENT = 'pathkeep-find-current'

export function canPaint() {
  return typeof CSS !== 'undefined' && 'highlights' in CSS
}

function pattern(words: string) {
  const escaped = words
    .trim()
    .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    .replace(/\s+/g, '\\s+')
  return escaped ? new RegExp(escaped, 'gi') : null
}

/** Ranges of every place `words` appears in the text under `element`. */
function rangesOf(element: Element, words: string): Range[] {
  const regex = pattern(words)
  if (!regex) return []
  const nodes: { node: Text; start: number }[] = []
  let text = ''
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    nodes.push({ node: node as Text, start: text.length })
    text += (node as Text).data
  }
  const at = (offset: number, end: boolean) => {
    let low = 0
    let high = nodes.length - 1
    while (low < high) {
      const middle = (low + high + 1) >> 1
      const start = nodes[middle].start
      if (start < offset || (!end && start === offset)) low = middle
      else high = middle - 1
    }
    const entry = nodes[low]
    return { node: entry.node, offset: offset - entry.start }
  }
  const ranges: Range[] = []
  for (const found of text.matchAll(regex)) {
    if (!found[0]) continue
    const start = at(found.index, false)
    const end = at(found.index + found[0].length, true)
    const range = document.createRange()
    range.setStart(start.node, start.offset)
    range.setEnd(end.node, end.offset)
    ranges.push(range)
  }
  return ranges
}

/**
 * Paints the occurrences in `root`. Returns the current occurrence's range
 * when it is drawn, so the caller can bring it into view.
 */
export function paintFind(
  roots: Element[],
  matches: FoundMatches,
  current: SearchMatch | null,
): Range | null {
  if (!canPaint()) return null
  const all: Range[] = []
  let currentRange: Range | null = null
  const currentToken = current ? partToken(current) : null
  for (const root of roots)
    for (const element of root.querySelectorAll('[data-part]')) {
      const tokens = (element.getAttribute('data-part') ?? '').split(' ')
      const here = tokens.flatMap((token) => matches.byToken.get(token) ?? [])
      if (here.length === 0) continue
      const byWords = new Map<string, Range[]>()
      for (const match of here) {
        const words = matchedText(match).toLowerCase()
        if (!byWords.has(words)) {
          const ranges = rangesOf(element, words)
          byWords.set(words, ranges)
          all.push(...ranges)
        }
      }
      if (current && currentToken && tokens.includes(currentToken)) {
        const words = matchedText(current).toLowerCase()
        const before = here.filter(
          (match) =>
            match.occurrence < current.occurrence &&
            matchedText(match).toLowerCase() === words,
        ).length
        const ranges = byWords.get(words) ?? []
        currentRange = ranges[Math.min(before, ranges.length - 1)] ?? null
      }
    }
  CSS.highlights.set(ALL, new Highlight(...all))
  if (currentRange) CSS.highlights.set(CURRENT, new Highlight(currentRange))
  else CSS.highlights.delete(CURRENT)
  return currentRange
}

export function clearFind() {
  if (!canPaint()) return
  CSS.highlights.delete(ALL)
  CSS.highlights.delete(CURRENT)
}
