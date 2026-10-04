/**
 * The Firefox history file that `browser-import.spec` imports through
 * Browser Direct: a separate places.sqlite, as if copied from another
 * computer. Visits are derived from the fixture's `now`, so a repeated run
 * writes the same file, and `wipe.spec` can count what the import left in
 * the archive.
 */
import path from 'node:path'
import type { FixtureVisit } from './fixture'
import { fixture } from './fixture'

/** Every title starts with this, so one search finds exactly these visits. */
export const BROWSER_FILE_TITLE = 'Old laptop notebook'

const HOUR_MS = 3_600_000
const DAY_MS = 24 * HOUR_MS

export function browserFilePath() {
  return path.join(fixture().root, 'old-laptop', 'places.sqlite')
}

function visit(page: number, at: number): FixtureVisit {
  return {
    url: `https://old-laptop.e2e.example/notebook/${page}`,
    title: `${BROWSER_FILE_TITLE} ${page}`,
    at,
    typed: false,
    duration: 20_000,
  }
}

/** What the file holds at first: 14 visits to 4 pages, 40 days ago. */
export function firstBrowserFileVisits(): FixtureVisit[] {
  const start = fixture().now - 40 * DAY_MS
  return Array.from({ length: 14 }, (_, index) =>
    visit(index % 4, start + index * HOUR_MS),
  )
}

/** Browsing added to the same file later: 2 on a known page, 3 on a new one. */
export function laterBrowserFileVisits(): FixtureVisit[] {
  const start = fixture().now - 20 * DAY_MS
  return [
    visit(1, start),
    visit(1, start + HOUR_MS),
    visit(9, start + 2 * HOUR_MS),
    visit(9, start + 3 * HOUR_MS),
    visit(9, start + 4 * HOUR_MS),
  ]
}

/** Visits the archive shows once browser-import.spec has finished (all restored). */
export function browserFileVisitCount() {
  return firstBrowserFileVisits().length + laterBrowserFileVisits().length
}
