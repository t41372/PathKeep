/**
 * The Google Takeout folders `takeout-import.spec` imports: Chrome's
 * `BrowserHistory.json` in the shape Takeout writes it (a `Browser History`
 * array, times in Unix microseconds), plus files that must stay out: My
 * Activity's Chrome entries in HTML (Takeout's default), Search's My Activity,
 * JSON Lines and the archive index; My Activity's Chrome entries in JSON, in a
 * Traditional Chinese account's folder; and a download as Google sends it, one
 * .zip with Chrome's history and My Activity's copy of it. Times come from the
 * fixture's `now`, so a repeated run writes the same files, and `wipe.spec`
 * can count what the imports left in the archive.
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { archivedVisits, fixture } from './fixture'
import { zipBytes } from './zip-file'

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
  // Another product's My Activity, in JSON and with the same title prefix:
  // searches are not visits, so if it were imported the search would count it.
  mkdirSync(path.join(root, 'My Activity', 'Search'), { recursive: true })
  writeFileSync(
    path.join(root, 'My Activity', 'Search', 'MyActivity.json'),
    JSON.stringify(
      [0, 1].map((index) => ({
        header: 'Search',
        title: `Searched for ${TAKEOUT_TITLE} ${index}`,
        titleUrl: `https://www.google.com/search?q=takeout+ledger+${index}`,
        time: new Date(
          fixture().now - 40 * DAY_MS + index * HOUR_MS,
        ).toISOString(),
        products: ['Search'],
      })),
    ),
  )
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

/**
 * My Activity copies of every visit the browsers recorded to `url`, as
 * Takeout writes them: the same page and instant, behind Google's redirect.
 * They are the same visits, so importing them adds nothing that counts.
 */
export function writeMyActivityCopiesFolder(url: string) {
  const root = path.join(fixture().root, 'Takeout-my-activity-copies')
  const folder = path.join(root, 'My Activity', 'Chrome')
  mkdirSync(folder, { recursive: true })
  const copies = archivedVisits()
    .filter((visit) => visit.url === url)
    .sort((a, b) => b.at - a.at)
    .map((visit) => ({
      title: `Visited ${visit.title}`,
      titleUrl: `https://www.google.com/url?q=${encodeURIComponent(visit.url)}&usg=AOvVaw0e2e`,
      time: new Date(visit.at).toISOString(),
      products: ['Chrome'],
    }))
  writeFileSync(path.join(folder, 'MyActivity.json'), JSON.stringify(copies))
  return { root, copies: copies.length }
}

/** Every title in the .zip below starts with this. */
export const ZIP_TITLE = 'Zipped parcel'

/** Four visits to two pages, 70 days ago, on a site no other spec visits. */
function zipVisits(): TakeoutVisit[] {
  const start = fixture().now - 70 * DAY_MS
  return Array.from({ length: 4 }, (_, index) => ({
    url: `https://parcel.e2e.example/item/${index % 2}`,
    title: `${ZIP_TITLE} ${index % 2}`,
    at: start + index * HOUR_MS,
  }))
}

export function zipVisitCount() {
  return zipVisits().length
}

export function zipPageCount() {
  return new Set(zipVisits().map((visit) => visit.url)).size
}

/**
 * A Takeout download with Chrome and My Activity ticked, as one .zip: a
 * `Takeout` folder holding Chrome's history under its current name
 * (`History.json`) next to Chrome's other files, and My Activity's Chrome
 * entries in JSON, which repeat every visit (same page and instant, behind
 * Google's redirect). My Activity's folder has the name an Indonesian account
 * gets, which sorts before "Chrome", and the zip lists it first too. Chrome's
 * bookmarks and search engines carry the title, so if they were read as visits
 * the search would count them.
 */
export function writeTakeoutZip() {
  const visits = zipVisits()
  const history = {
    'Browser History': visits.map((visit) => ({
      favicon_url: '',
      page_transition: 'LINK',
      title: visit.title,
      url: visit.url,
      client_id: 'e2e-zip-client',
      time_usec: visit.at * 1000,
    })),
  }
  const activity = [...visits]
    .sort((a, b) => b.at - a.at)
    .map((visit) => ({
      header: 'parcel.e2e.example',
      title: `Visited ${visit.title}`,
      titleUrl: `https://www.google.com/url?q=${encodeURIComponent(visit.url)}&usg=AOvVaw0e2e`,
      time: new Date(visit.at).toISOString(),
      products: ['Chrome'],
    }))
  const saved = Math.floor((fixture().now - 80 * DAY_MS) / 1000)
  const bookmarks = [
    '<!DOCTYPE NETSCAPE-Bookmark-file-1>',
    '<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">',
    '<TITLE>Bookmarks</TITLE>',
    '<H1>Bookmarks</H1>',
    '<DL><p>',
    `<DT><A HREF="https://parcel.e2e.example/saved" ADD_DATE="${saved}">${ZIP_TITLE} bookmark</A>`,
    '</DL><p>',
  ].join('\n')
  const engines = {
    'Search Engines': [
      {
        short_name: `${ZIP_TITLE} search`,
        keyword: 'parcel',
        url: 'https://parcel.e2e.example/search?q={searchTerms}',
        suggestions_url: '',
        favicon_url: '',
      },
    ],
  }
  const file = path.join(fixture().root, 'takeout-20261009T101500Z-001.zip')
  writeFileSync(
    file,
    zipBytes([
      [
        'Takeout/Aktivitas Saya/Chrome/AktivitasSaya.json',
        JSON.stringify(activity),
      ],
      ['Takeout/Chrome/Bookmarks.html', bookmarks],
      ['Takeout/Chrome/History.json', JSON.stringify(history)],
      ['Takeout/Chrome/Search Engines.json', JSON.stringify(engines)],
      ['Takeout/archive_browser.html', '<html></html>'],
    ]),
  )
  return file
}
