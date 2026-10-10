/**
 * The Google Takeout folders `takeout-import.spec` imports: Chrome's
 * `BrowserHistory.json` in the shape Takeout writes it (a `Browser History`
 * array, times in Unix microseconds), plus files that must stay out: My
 * Activity's Chrome entries in HTML (Takeout's default), JSON Lines and the
 * archive index; and My Activity's Chrome entries in JSON, in a
 * Traditional Chinese account's folder. Times
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

/**
 * Six visits to three pages, 60 days ago, on a site no other spec visits, so
 * its Insights page holds only what Takeout recorded.
 */
function takeoutVisits(): TakeoutVisit[] {
  const start = fixture().now - 60 * DAY_MS
  return Array.from({ length: 6 }, (_, index) => ({
    url: `https://takeout.example/ledger/${index % 3}`,
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
  writeMyActivity(root)
  return root
}

/**
 * My Activity → Chrome, as Takeout writes it by default (HTML). Its entries
 * carry the same title prefix, so if they were imported the search would
 * count them.
 */
function writeMyActivity(root: string) {
  mkdirSync(path.join(root, 'My Activity', 'Chrome'), { recursive: true })
  const entries = [0, 1]
    .map(
      (index) =>
        `<div class="outer-cell"><div class="content-cell">Visited <a href="https://activity.e2e.example/${index}">${TAKEOUT_TITLE} activity ${index}</a><br>Oct 1, 2026, 9:0${index}:00 AM UTC</div></div>`,
    )
    .join('')
  writeFileSync(
    path.join(root, 'My Activity', 'Chrome', 'MyActivity.html'),
    `<html><body>${entries}</body></html>`,
  )
}

/** A Takeout folder with only My Activity's Chrome entries. */
export function writeMyActivityOnlyFolder() {
  const root = path.join(fixture().root, 'Takeout-my-activity')
  writeMyActivity(root)
  return root
}

/** Every My Activity title starts with this, after Google's "Visited " prefix. */
export const ACTIVITY_TITLE = 'Activity ledger'

/**
 * Four visits to two pages, 50 days ago, on a site no other spec visits. Two
 * are behind Google's click redirect, as Takeout writes most of them.
 */
function activityVisits(): (TakeoutVisit & { redirected: boolean })[] {
  const start = fixture().now - 50 * DAY_MS
  return Array.from({ length: 4 }, (_, index) => ({
    url: `https://activity.e2e.example/page/${index % 2}`,
    title: `${ACTIVITY_TITLE} ${index % 2}`,
    at: start + index * HOUR_MS,
    redirected: index % 2 === 0,
  }))
}

export function activityVisitCount() {
  return activityVisits().length
}

export function activityPageCount() {
  return new Set(activityVisits().map((visit) => visit.url)).size
}

/**
 * My Activity → Chrome in JSON, under the folder and file names a Traditional
 * Chinese account gets: the visits above, newest first as Takeout lists them,
 * and a "Used Chrome" record that is not a visit.
 */
export function writeMyActivityJsonFolder() {
  const root = path.join(fixture().root, 'Takeout-my-activity-json')
  const folder = path.join(root, '我的活動', 'Chrome')
  mkdirSync(folder, { recursive: true })
  const records = activityVisits()
    .map((visit) => ({
      header: 'activity.e2e.example',
      title: `Visited ${visit.title}`,
      titleUrl: visit.redirected
        ? `https://www.google.com/url?q=${encodeURIComponent(visit.url)}&usg=AOvVaw0e2e`
        : visit.url,
      time: new Date(visit.at).toISOString(),
      products: ['Chrome'],
    }))
    .reverse()
  records.push({
    header: 'Chrome',
    title: 'Used Chrome',
    time: new Date(fixture().now - 51 * DAY_MS).toISOString(),
    products: ['Chrome'],
  } as (typeof records)[number])
  writeFileSync(path.join(folder, '我的活動.json'), JSON.stringify(records))
  return root
}
