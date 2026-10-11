/**
 * The Safari history file `browser-import.spec` imports: a `History.db`
 * copied from another Mac. Safari keeps a title on each visit, not on the
 * page, and leaves some visits untitled. Visits are at local midday on days
 * derived from the fixture's `now`, so a repeated run writes the same file,
 * each visit's day is the same whatever the time zone, and `wipe.spec` can
 * count what the import left in the archive.
 */
import path from 'node:path'
import { fixture } from './fixture'

export { localDay } from './fixture'

/** The title Safari last saw on every page, so one search finds them all. */
export const SAFARI_TITLE = 'Mac mini almanac'

const HOUR_MS = 3_600_000
const DAY_MS = 24 * HOUR_MS

export interface SafariVisit {
  url: string
  title: string | null
  at: number
}

export function safariFilePath() {
  return path.join(fixture().root, 'old-mac', 'History.db')
}

/** Local midday `daysAgo` days before the fixture's `now`. */
function midday(daysAgo: number) {
  const day = new Date(fixture().now - daysAgo * DAY_MS)
  day.setHours(12, 0, 0, 0)
  return day.getTime()
}

/** A site of its own: under e2e.example it would share the site e2e.example with other specs' pages. */
const page = (index: number) => `https://almanac.example/entry/${index}`

/**
 * Five visits to three pages on two days. Page 0 was first titled otherwise;
 * page 1's latest visit has no title, so its name is the one before.
 */
export function safariVisits(): SafariVisit[] {
  const first = midday(35)
  const second = midday(34)
  return [
    { url: page(0), title: 'Almanac draft', at: first },
    { url: page(1), title: `${SAFARI_TITLE} 1`, at: first + HOUR_MS },
    { url: page(0), title: `${SAFARI_TITLE} 0`, at: first + 2 * HOUR_MS },
    { url: page(2), title: `${SAFARI_TITLE} 2`, at: second },
    { url: page(1), title: null, at: second + HOUR_MS },
  ]
}

export function safariVisitCount() {
  return safariVisits().length
}
