/**
 * The Google Takeout folder `takeout-import.spec` imports: Chrome's
 * `BrowserHistory.json` in the shape Takeout writes it (a `Browser History`
 * array, times in Unix microseconds), plus files that must stay out. Times
 * come from the fixture's `now`, so a repeated run writes the same folder,
 * and `wipe.spec` can count what the import left in the archive.
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fixture } from './fixture'

/** Every imported title starts with this, so one search finds exactly them. */
export const TAKEOUT_TITLE = 'Takeout ledger'

const HOUR_MS = 3_600_000
const DAY_MS = 24 * HOUR_MS

interface TakeoutVisit {
  url: string
  title: string
  at: number
}

/** Six visits to three pages, 60 days ago. */
function takeoutVisits(): TakeoutVisit[] {
  const start = fixture().now - 60 * DAY_MS
  return Array.from({ length: 6 }, (_, index) => ({
    url: `https://takeout.e2e.example/ledger/${index % 3}`,
    title: `${TAKEOUT_TITLE} ${index % 3}`,
    at: start + index * HOUR_MS,
  }))
}

export function takeoutVisitCount() {
  return takeoutVisits().length
}

export function takeoutPageCount() {
  return new Set(takeoutVisits().map((visit) => visit.url)).size
}

/** Writes the folder and returns its path. */
export function writeTakeoutFolder() {
  const root = path.join(fixture().root, 'Takeout')
  mkdirSync(path.join(root, 'Chrome'), { recursive: true })
  writeFileSync(
    path.join(root, 'Chrome', 'BrowserHistory.json'),
    JSON.stringify({
      'Browser History': takeoutVisits().map((visit) => ({
        favicon_url: '',
        page_transition: 'LINK',
        title: visit.title,
        url: visit.url,
        client_id: 'e2e-takeout-client',
        time_usec: visit.at * 1000,
      })),
    }),
  )
  // History-shaped lines with the same title prefix. Takeout never writes
  // JSON Lines, so if these were imported the search would count them.
  writeFileSync(
    path.join(root, 'history-export.jsonl'),
    [0, 1, 2]
      .map((index) =>
        JSON.stringify({
          url: `https://stray.e2e.example/${index}`,
          title: `${TAKEOUT_TITLE} stray ${index}`,
          time_usec: (fixture().now - 30 * DAY_MS + index * HOUR_MS) * 1000,
        }),
      )
      .join('\n'),
  )
  writeFileSync(path.join(root, 'archive_browser.html'), '<html></html>')
  return root
}
