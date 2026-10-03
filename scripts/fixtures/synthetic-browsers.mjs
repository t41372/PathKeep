/**
 * Writes synthetic but realistic browser profiles to disk: two Chrome
 * profiles and one Firefox profile, with real SQLite history databases.
 *
 * The desktop bridge (dev and E2E) points PathKeep's discovery at these
 * folders through CHB_CHROME_USER_DATA_DIR / CHB_FIREFOX_PROFILES_DIR, so
 * the real Rust backup path copies, parses and archives them.
 *
 * Output is deterministic for a given seed and `now`, so test runs repeat.
 */
import { DatabaseSync } from 'node:sqlite'
import { mkdirSync, writeFileSync } from 'node:fs'
import path from 'node:path'

const DAY_MS = 86_400_000
const CHROME_EPOCH_OFFSET_US = 11_644_473_600_000_000

const topics = [
  {
    name: 'rust',
    search: [
      'tokio spawn_blocking',
      'rust async runtime',
      'tokio vs smol',
      'rust pin explained',
    ],
    pages: [
      [
        'https://docs.rs/tokio/latest/tokio/runtime/struct.Builder.html',
        'tokio::runtime::Builder',
      ],
      ['https://docs.rs/smol/latest/smol/', 'smol - Rust'],
      [
        'https://without.boats/blog/async-runtimes/',
        'tokio vs smol: choosing an async runtime',
      ],
      [
        'https://rust-lang.github.io/async-book/',
        'Async Rust: what is a runtime?',
      ],
      [
        'https://github.com/tokio-rs/tokio/issues/5124',
        'spawn_blocking pool exhaustion · tokio-rs/tokio',
      ],
      [
        'https://news.ycombinator.com/item?id=41502281',
        'Hacker News · Why I left tokio',
      ],
    ],
  },
  {
    name: 'camera',
    search: ['a7c ii autofocus', 'nikon z f review', 'fujifilm x-t5 vs a7c ii'],
    pages: [
      [
        'https://www.dpreview.com/reviews/sony-a7cii-vs-nikon-zf',
        'Sony A7C II vs Nikon Z f autofocus test',
      ],
      [
        'https://www.bhphotovideo.com/c/product/nikon-z-f',
        'Nikon Z f Mirrorless Camera Body | B&H',
      ],
      [
        'https://www.reddit.com/r/SonyAlpha/comments/a7cii_6_months',
        'r/SonyAlpha · A7C II after 6 months',
      ],
      [
        'https://www.dpreview.com/reviews/fujifilm-x-t5',
        'Fujifilm X-T5 review',
      ],
    ],
  },
  {
    name: 'running',
    search: ['half marathon training plan', 'easy run pace', 'marathon taper'],
    pages: [
      [
        'https://www.runnersworld.com/training/half-marathon-plan',
        'December half marathon training plan · 12 weeks',
      ],
      ['https://www.strava.com/activities/1001', 'Strava · Afternoon run'],
      ['https://www.strava.com/athlete/training', 'Strava · Training log'],
    ],
  },
  {
    name: 'japanese',
    search: [
      'jlpt n3 grammar',
      'n3 listening practice',
      'japanese particles wa ga',
    ],
    pages: [
      [
        'https://jlptsensei.com/n3-listening/14',
        'JLPT N3 listening practice set 14',
      ],
      ['https://jlptsensei.com/jlpt-n3-grammar-list/', 'JLPT N3 grammar list'],
      [
        'https://www.youtube.com/watch?v=n3listening',
        'N3 listening drills - YouTube',
      ],
    ],
  },
  {
    name: 'frontend',
    search: [
      'shadcn sidebar',
      'tauri window vibrancy',
      'tanstack virtual dynamic height',
    ],
    pages: [
      ['https://ui.shadcn.com/docs/components/sidebar', 'shadcn/ui · Sidebar'],
      [
        'https://v2.tauri.app/reference/javascript/api/namespacewindow/',
        'Tauri 2 · Window vibrancy on macOS',
      ],
      [
        'https://tanstack.com/virtual/latest/docs/introduction',
        'TanStack Virtual · Introduction',
      ],
      ['https://github.com/shadcn-ui/ui', 'shadcn-ui/ui · GitHub'],
    ],
  },
  {
    name: 'news',
    search: ['weather tokyo', 'sqlite fts5 chinese tokenizer'],
    pages: [
      ['https://news.ycombinator.com/', 'Hacker News'],
      ['https://www.sqlite.org/fts5.html', 'SQLite FTS5 Extension'],
      ['https://github.com/', 'GitHub'],
      ['https://www.youtube.com/', 'YouTube'],
    ],
  },
]

function prng(seed) {
  let state = seed >>> 0 || 1
  return () => {
    state = (state * 16807) % 2147483647
    return (state - 1) / 2147483646
  }
}

/**
 * Generates visits newest-day-first for `days` days ending at `now`.
 * Each day has a few sessions; each session dwells on one topic, starts
 * with a search now and then, and revisits pages so re-find stats exist.
 */
export function generateVisits({
  now = Date.now(),
  days = 420,
  perDay = 60,
  seed = 7,
} = {}) {
  const random = prng(seed)
  const visits = []
  const today = new Date(now)
  today.setHours(0, 0, 0, 0)
  for (let day = 0; day < days; day += 1) {
    const dayStart = today.getTime() - day * DAY_MS
    const weekend = [0, 6].includes(new Date(dayStart).getDay())
    const volume = Math.round(
      perDay * (weekend ? 0.55 : 1) * (0.6 + random() * 0.8),
    )
    const sessions = 2 + Math.floor(random() * 4)
    let produced = 0
    for (
      let session = 0;
      session < sessions && produced < volume;
      session += 1
    ) {
      const topic = topics[Math.floor(random() * topics.length)]
      const hour = [9, 10, 11, 14, 15, 16, 20, 21, 22][Math.floor(random() * 9)]
      let at = dayStart + hour * 3_600_000 + Math.floor(random() * 50) * 60_000
      if (at > now) continue
      const size = Math.max(2, Math.round(volume / sessions))
      if (random() < 0.7) {
        const term = topic.search[Math.floor(random() * topic.search.length)]
        visits.push({
          url: `https://www.google.com/search?q=${encodeURIComponent(term)}`,
          title: `${term} - Google Search`,
          at,
          typed: true,
          term,
          duration: 20_000,
        })
        produced += 1
      }
      for (let index = 0; index < size && produced < volume; index += 1) {
        at += 40_000 + Math.floor(random() * 240_000)
        if (at > now) break
        const [url, title] =
          topic.pages[Math.floor(random() * topic.pages.length)]
        visits.push({
          url,
          title,
          at,
          typed: random() < 0.08,
          duration: Math.floor(random() * 300_000),
        })
        produced += 1
      }
    }
  }
  return visits.sort((a, b) => a.at - b.at)
}

function writeChromeHistory(file, visits) {
  const db = new DatabaseSync(file)
  db.exec(`
    CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT, visit_count INTEGER NOT NULL,
      typed_count INTEGER NOT NULL, last_visit_time INTEGER NOT NULL, hidden INTEGER NOT NULL);
    CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER NOT NULL, visit_time INTEGER NOT NULL, from_visit INTEGER,
      transition INTEGER, visit_duration INTEGER, is_known_to_sync INTEGER, visited_link_id INTEGER,
      external_referrer_url TEXT, app_id TEXT);
    CREATE TABLE downloads (id INTEGER PRIMARY KEY, guid TEXT, current_path TEXT, target_path TEXT, start_time INTEGER,
      received_bytes INTEGER, total_bytes INTEGER, state INTEGER, mime_type TEXT, original_mime_type TEXT);
    CREATE TABLE keyword_search_terms (keyword_id INTEGER, url_id INTEGER, term TEXT, normalized_term TEXT);
    CREATE INDEX visits_url_index ON visits(url);
    CREATE INDEX visits_time_index ON visits(visit_time);
  `)
  const urlIds = new Map()
  const insertUrl = db.prepare(
    'INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden) VALUES (?, ?, ?, 0, 0, 0, 0)',
  )
  const insertVisit = db.prepare(
    'INSERT INTO visits (url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id) VALUES (?, ?, 0, ?, ?, 0, 0, NULL, NULL)',
  )
  const bump = db.prepare(
    'UPDATE urls SET visit_count = visit_count + 1, typed_count = typed_count + ?, last_visit_time = ? WHERE id = ?',
  )
  const insertTerm = db.prepare(
    'INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term) VALUES (2, ?, ?, ?)',
  )
  db.exec('BEGIN')
  for (const visit of visits) {
    let id = urlIds.get(visit.url)
    if (id === undefined) {
      id = urlIds.size + 1
      urlIds.set(visit.url, id)
      insertUrl.run(id, visit.url, visit.title)
      if (visit.term) insertTerm.run(id, visit.term, visit.term.toLowerCase())
    }
    const time = Math.trunc(visit.at * 1000 + CHROME_EPOCH_OFFSET_US)
    // 0x30000000 = chain start + chain end; core type 1 = typed, 0 = link.
    insertVisit.run(
      id,
      time,
      0x30000000 | (visit.typed ? 1 : 0),
      visit.duration * 1000,
    )
    bump.run(visit.typed ? 1 : 0, time, id)
  }
  db.exec('COMMIT')
  db.close()
}

function writeFirefoxPlaces(file, visits) {
  const db = new DatabaseSync(file)
  db.exec(`
    CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT, visit_count INTEGER, hidden INTEGER, last_visit_date INTEGER);
    CREATE TABLE moz_historyvisits (id INTEGER PRIMARY KEY, place_id INTEGER NOT NULL, visit_date INTEGER NOT NULL, from_visit INTEGER, visit_type INTEGER);
    CREATE INDEX moz_places_url_index ON moz_places(url);
    CREATE INDEX moz_historyvisits_place_index ON moz_historyvisits(place_id);
    CREATE INDEX moz_historyvisits_date_index ON moz_historyvisits(visit_date);
  `)
  const ids = new Map()
  const insertPlace = db.prepare(
    'INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date) VALUES (?, ?, ?, 0, 0, 0)',
  )
  const insertVisit = db.prepare(
    'INSERT INTO moz_historyvisits (place_id, visit_date, from_visit, visit_type) VALUES (?, ?, 0, ?)',
  )
  const bump = db.prepare(
    'UPDATE moz_places SET visit_count = visit_count + 1, last_visit_date = ? WHERE id = ?',
  )
  db.exec('BEGIN')
  for (const visit of visits) {
    let id = ids.get(visit.url)
    if (id === undefined) {
      id = ids.size + 1
      ids.set(visit.url, id)
      insertPlace.run(id, visit.url, visit.title)
    }
    const time = Math.trunc(visit.at * 1000)
    insertVisit.run(id, time, visit.typed ? 2 : 1)
    bump.run(time, id)
  }
  db.exec('COMMIT')
  db.close()
}

/**
 * Creates the fixture tree under `root` and returns the paths to export as
 * environment variables. `scale` multiplies the per-day volume.
 */
export function writeSyntheticBrowsers(
  root,
  { now = Date.now(), days = 420, perDay = 60, seed = 7 } = {},
) {
  const chromeRoot = path.join(root, 'chrome-user-data')
  const firefoxRoot = path.join(root, 'firefox', 'Profiles')
  mkdirSync(path.join(chromeRoot, 'Default'), { recursive: true })
  mkdirSync(path.join(chromeRoot, 'Profile 1'), { recursive: true })
  mkdirSync(path.join(firefoxRoot, 'k3x9.default-release'), { recursive: true })

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
  writeFileSync(
    path.join(root, 'firefox', 'profiles.ini'),
    '[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/k3x9.default-release\nDefault=1\n',
  )

  const all = generateVisits({ now, days, perDay, seed })
  // Split by topic-ish hash so each browser has a recognisable mix.
  const personal = []
  const work = []
  const firefox = []
  for (const visit of all) {
    const bucket = (visit.url.length + Math.floor(visit.at / DAY_MS)) % 5
    if (bucket < 2) personal.push(visit)
    else if (bucket < 4) work.push(visit)
    else firefox.push(visit)
  }
  writeChromeHistory(path.join(chromeRoot, 'Default', 'History'), personal)
  writeChromeHistory(path.join(chromeRoot, 'Profile 1', 'History'), work)
  writeFirefoxPlaces(
    path.join(firefoxRoot, 'k3x9.default-release', 'places.sqlite'),
    firefox,
  )

  return {
    chromeUserDataRoot: chromeRoot,
    firefoxProfilesRoot: firefoxRoot,
    counts: {
      total: all.length,
      personal: personal.length,
      work: work.length,
      firefox: firefox.length,
    },
  }
}
