/**
 * The Firefox history file that `browser-import.spec` imports through
 * Browser Direct: a separate places.sqlite, as if copied from another
 * computer. Visits are derived from the fixture's `now`, so a repeated run
 * writes the same file, and `wipe.spec` can count what the import left in
 * the archive. Also the larger file `import-stop.spec` stops half way.
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

/**
 * The visits of the Firefox file `import-stop.spec` stops half way and then
 * imports in full: five of the import's 10,000-visit parts, 300 pages on a
 * site of its own, about 600 days ago.
 */
export function largeBrowserFileVisits(): FixtureVisit[] {
  const start = fixture().now - 600 * DAY_MS
  return Array.from({ length: 50_000 }, (_, index) => ({
    url: `https://stopped-laptop.e2e.example/page/${index % 300}`,
    title: `Stopped laptop page ${index % 300}`,
    at: start + index * 60_000,
    typed: false,
    duration: 20_000,
  }))
}

export function largeBrowserFilePath() {
  return path.join(fixture().root, 'stopped-laptop', 'places.sqlite')
}

/** Every title of the file `browser-import.spec` imports and then undoes starts with this. */
export const UNDONE_FILE_TITLE = 'Undo check page'
export const UNDONE_FILE_SITE = 'undo-check.example'

/** A Chrome History of its own, so Insights lists its profile apart from the Firefox file's. */
export function undoneBrowserFilePath() {
  return path.join(fixture().root, 'undo-laptop', 'History')
}

/**
 * The visits of the file `browser-import.spec` imports and then undoes: 24
 * visits to three pages on a site of its own, every 20 minutes from 08:00 on
 * one day 120 days back, a day the other specs add nothing to.
 */
export function undoneBrowserFileVisits(): FixtureVisit[] {
  const start = new Date(fixture().now - 120 * DAY_MS)
  start.setHours(8, 0, 0, 0)
  return Array.from({ length: 24 }, (_, index) => ({
    url: `https://${UNDONE_FILE_SITE}/page/${index % 3}`,
    title: `${UNDONE_FILE_TITLE} ${index % 3}`,
    at: start.getTime() + index * 20 * 60_000,
    typed: true,
    duration: 30_000,
  }))
}
