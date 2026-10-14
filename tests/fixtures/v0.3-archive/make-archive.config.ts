/**
 * Runs v0.3.0's own desktop app (dev bridge) on synthetic Chrome profiles so
 * tests/e2e/v03-archive.spec.ts can make a v0.3.0 data folder with it.
 *
 * The browser data is deterministic: a year of visits over two profiles, ending
 * shortly before the run, plus a third Chrome file for a browser-file import.
 */
import { DatabaseSync } from 'node:sqlite'
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { defineConfig, devices } from '@playwright/test'
import { resolveDesktopBridgeEnv } from './scripts/pathkeep-dev-desktop-bridge.mjs'

const DAY = 86_400_000

function chromeMicros(ms: number) {
  return Math.trunc(ms * 1_000 + 11_644_473_600_000_000)
}

/** A small deterministic generator, so every run writes the same history. */
function random(seed: number) {
  let state = seed >>> 0
  return () => {
    state = (state * 1_664_525 + 1_013_904_223) >>> 0
    return state / 2 ** 32
  }
}

interface Visit {
  url: string
  title: string
  at: number
  transition: number
  search?: string
}

const LINK = 0
const TYPED = 1

function writeHistory(file: string, visits: Visit[]) {
  const db = new DatabaseSync(file)
  db.exec(`
    CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT,
      visit_count INTEGER NOT NULL, typed_count INTEGER NOT NULL,
      last_visit_time INTEGER NOT NULL, hidden INTEGER NOT NULL);
    CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER NOT NULL,
      visit_time INTEGER NOT NULL, from_visit INTEGER, transition INTEGER,
      visit_duration INTEGER, is_known_to_sync INTEGER, visited_link_id INTEGER,
      external_referrer_url TEXT, app_id TEXT);
    CREATE TABLE downloads (id INTEGER PRIMARY KEY, guid TEXT, current_path TEXT,
      target_path TEXT, start_time INTEGER, received_bytes INTEGER,
      total_bytes INTEGER, state INTEGER, mime_type TEXT, original_mime_type TEXT);
    CREATE TABLE keyword_search_terms (keyword_id INTEGER, url_id INTEGER,
      term TEXT, normalized_term TEXT);
  `)
  const urlIds = new Map<string, number>()
  const insertUrl = db.prepare(
    'INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden) VALUES (?, ?, ?, 0, 0, 0, 0)',
  )
  const bump = db.prepare(
    'UPDATE urls SET visit_count = visit_count + 1, typed_count = typed_count + ?, last_visit_time = MAX(last_visit_time, ?) WHERE id = ?',
  )
  const insertVisit = db.prepare(
    'INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id) VALUES (?, ?, ?, ?, ?, ?, 1, 0, NULL, NULL)',
  )
  const insertTerm = db.prepare(
    'INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term) VALUES (2, ?, ?, ?)',
  )
  const sorted = [...visits].sort((a, b) => a.at - b.at)
  let previous: { id: number; at: number } | null = null
  sorted.forEach((visit, index) => {
    let urlId = urlIds.get(visit.url)
    if (!urlId) {
      urlId = urlIds.size + 1
      urlIds.set(visit.url, urlId)
      insertUrl.run(urlId, visit.url, visit.title)
      if (visit.search)
        insertTerm.run(urlId, visit.search, visit.search.toLowerCase())
    }
    const at = chromeMicros(visit.at)
    bump.run(visit.transition === TYPED ? 1 : 0, at, urlId)
    // A link within 30 minutes of the previous visit follows from it.
    const from =
      visit.transition === LINK &&
      previous &&
      visit.at - previous.at < 1_800_000
        ? previous.id
        : null
    // Chrome keeps the core type in the low byte plus chain-end flags.
    insertVisit.run(
      index + 1,
      urlId,
      at,
      from,
      0x30000000 | visit.transition,
      20_000_000,
    )
    previous = { id: index + 1, at: visit.at }
  })
  db.close()
}

/** A year of everyday browsing, ending a day before `end`. */
function everyday(seed: number, end: number, count: number, sites: string[]) {
  const next = random(seed)
  const visits: Visit[] = []
  const searches = [
    'rust ownership',
    'Rust Ownership',
    'tide tables coast',
    'quokka habitat',
    'sqlcipher wal mode',
    '袋鼠 習性',
  ]
  for (let i = 0; i < count; i += 1) {
    const at = end - DAY - Math.floor(next() * 365 * DAY)
    if (i % 12 === 0) {
      const term = searches[Math.floor(next() * searches.length)]
      visits.push({
        url: `https://www.google.com/search?q=${encodeURIComponent(term).replace(/%20/g, '+')}`,
        title: `${term} - Google Search`,
        at,
        transition: LINK,
        search: term,
      })
      continue
    }
    const site = sites[Math.floor(next() * sites.length)]
    const page = Math.floor(next() * 40)
    visits.push({
      url: `https://${site}/page/${page}`,
      title: `${site} page ${page}`,
      at,
      transition: next() < 0.15 ? TYPED : LINK,
    })
  }
  return visits
}

function prepareFixture() {
  const root = mkdtempSync(path.join(os.tmpdir(), 'pathkeep-v03-'))
  const projectRoot = path.join(root, 'project-root')
  const chromeRoot = path.join(root, 'chrome-user-data')
  const keyringRoot = path.join(root, 'keyring')
  const importDir = path.join(root, 'import')
  for (const dir of [projectRoot, keyringRoot, importDir])
    mkdirSync(dir, { recursive: true })
  for (const profile of ['Default', 'Profile 1'])
    mkdirSync(path.join(chromeRoot, profile), { recursive: true })
  writeFileSync(path.join(chromeRoot, 'Last Version'), '135.0.0.0')
  writeFileSync(
    path.join(chromeRoot, 'Local State'),
    JSON.stringify({
      profile: {
        info_cache: {
          Default: { name: 'Personal', user_name: 'fixture@example.test' },
          'Profile 1': { name: 'Work', user_name: 'work@example.test' },
        },
      },
    }),
  )

  const end = Date.now() - 3_600_000
  const personalSites = Array.from(
    { length: 60 },
    (_, i) => `site${i}.example.net`,
  )
  const workSites = Array.from({ length: 30 }, (_, i) => `work${i}.example.net`)
  const personal = everyday(2025, end, 1_400, personalSites)
  personal.push(
    // Two spellings of one page (the old canonical rule kept `%7E`).
    {
      url: 'https://example.org/%7Equokka?utm_source=old',
      title: 'Quokka field notes',
      at: end - 40 * DAY,
      transition: LINK,
    },
    {
      url: 'https://example.org/%7Equokka?utm_source=old',
      title: 'Quokka field notes',
      at: end - 3 * DAY,
      transition: TYPED,
    },
    {
      url: 'https://example.org/~quokka',
      title: 'Quokka field notes',
      at: end - 2 * DAY,
      transition: LINK,
    },
    // Public pages v0.3.0 can fetch for saved page text.
    {
      url: 'https://example.com/',
      title: 'Example Domain',
      at: end - 5 * DAY,
      transition: TYPED,
    },
    {
      url: 'https://www.iana.org/help/example-domains',
      title: 'Example Domains',
      at: end - 4 * DAY,
      transition: LINK,
    },
    {
      url: 'https://zh.example.net/文章/潮汐',
      title: '潮汐表與袋鼠',
      at: end - 6 * DAY,
      transition: LINK,
    },
    {
      url: 'https://zh.example.net/文章/潮汐',
      title: '潮汐表與袋鼠',
      at: end - 1 * DAY,
      transition: LINK,
    },
  )
  const work = everyday(2026, end, 400, workSites)
  work.push({
    url: 'https://tides.example.net/coast',
    title: 'Tide tables',
    at: end - 7 * DAY,
    transition: TYPED,
  })
  writeHistory(path.join(chromeRoot, 'Default', 'History'), personal)
  writeHistory(path.join(chromeRoot, 'Profile 1', 'History'), work)
  // An old laptop's Chrome file, imported by hand.
  const laptop = everyday(2027, end - 400 * DAY, 120, [
    'laptop.example.net',
    'site3.example.net',
  ])
  writeHistory(path.join(importDir, 'History'), laptop)

  process.env.CHB_PROJECT_ROOT = projectRoot
  process.env.CHB_CHROME_USER_DATA_DIR = chromeRoot
  process.env.CHB_TEST_KEYRING_DIR = keyringRoot
  process.env.V03_IMPORT_FILE = path.join(importDir, 'History')
  process.env.V03_SEEDED = JSON.stringify({
    personal: personal.length,
    work: work.length,
    laptop: laptop.length,
  })
  process.env.CARGO_TARGET_DIR = path.join(
    process.cwd(),
    'var',
    'playwright',
    'desktop-bridge',
    'cargo-target',
  )
}

// The runner loads this file first and its workers inherit the environment;
// a worker must reuse the runner's folders and ports, not make new ones.
if (!process.env.CHB_PROJECT_ROOT) prepareFixture()
const devServerPort =
  process.env.PATHKEEP_DEV_SERVER_PORT ??
  String(15_420 + Math.trunc(Math.random() * 2_000))
const devIpcPort =
  process.env.PATHKEEP_DEV_IPC_PORT ??
  String(43_118 + Math.trunc(Math.random() * 2_000))
process.env.PATHKEEP_DEV_SERVER_PORT = devServerPort
process.env.PATHKEEP_DEV_IPC_PORT = devIpcPort
const bridgeEnv = resolveDesktopBridgeEnv({
  ...process.env,
  PATHKEEP_DEV_SERVER_PORT: devServerPort,
  PATHKEEP_DEV_IPC_PORT: devIpcPort,
})

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: 'v03-archive.spec.ts',
  timeout: 1_800_000,
  fullyParallel: false,
  retries: 0,
  reporter: [['list']],
  use: { baseURL: bridgeEnv.devServerUrl },
  webServer: {
    command: `PATHKEEP_DEV_SERVER_PORT=${devServerPort} PATHKEEP_DEV_IPC_PORT=${devIpcPort} bun run desktop:dev:bridge`,
    url: bridgeEnv.devServerUrl,
    reuseExistingServer: false,
    timeout: 1_800_000,
  },
  projects: [{ name: 'v03', use: { ...devices['Desktop Chrome'] } }],
})
