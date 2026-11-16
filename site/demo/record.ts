/**
 * Makes the demo's recording: writes synthetic browsers, runs the real
 * backend over them with its dev bridge, sets the archive up the way
 * onboarding's defaults do, then walks the app (the `--mode record` build)
 * through `walk.ts` in Chrome and keeps every command, answer and search.
 *
 *   node site/demo/record.ts [--backend <pathkeep-desktop>] [--out <folder>] [--shots <dir>]
 *
 * The backend is a debug build with the dev bridge
 * (`cargo build --manifest-path src-tauri/Cargo.toml --features devtools-bridge`),
 * its `pathkeep-worker` beside it. Record from a clean checkout of the
 * release: the About screen shows the build's version and commit.
 *
 * Everything runs in a fresh folder under the system's temp folder, never
 * in your own PathKeep data, and is removed at the end. A debug PathKeep
 * window may open while it runs; the walk does not use it.
 *
 * The answers carry the folders the backend ran in. Those are rewritten to
 * the places a Mac keeps them, and the run fails if anything about this
 * computer is left (home folder, user name, host name, Git e-mail, this
 * checkout, its time zone). Everything runs in one fixed time zone for the
 * same reason: Insights keep days in the computer's zone and say which.
 */
import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import {
  constants,
  copyFileSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { createServer } from 'node:http'
import { homedir, hostname, tmpdir, userInfo } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from '@playwright/test'
import { writeSyntheticBrowsers } from '../../scripts/fixtures/synthetic-browsers.mjs'
import type { SearchHit } from '../../src/lib/backend-client/search.ts'
import type {
  HistoryEntry,
  HistoryQueryResponse,
} from '../../src/lib/types/archive.ts'
import type { RecordedCall, Recording } from './backend.ts'
import type { RecordLog } from './record/log.ts'
import { shardOf } from './shards.ts'
import { packTimeline, unpackTimeline } from './timeline.ts'
import { walk } from './walk.ts'

/**
 * The demo archive's time zone, for the synthetic browsing (written in
 * local hours), the backend and the browser alike. The synthetic person
 * looks up Taipei's weather, so they live there.
 */
const zone = 'Asia/Taipei'
const ownZone = Intl.DateTimeFormat().resolvedOptions().timeZone
process.env.TZ = zone

const here = path.dirname(fileURLToPath(import.meta.url))
const repo = path.resolve(here, '../..')
const argv = process.argv.slice(2)
const flag = (name: string) =>
  argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined
const backend = path.resolve(
  flag('--backend') ??
    path.join(repo, 'var/demo-target/debug/pathkeep-desktop'),
)
const out = path.resolve(flag('--out') ?? path.join(here, 'public/recording'))
const shots = flag('--shots')

const root = path.join(tmpdir(), 'pathkeep-demo-record')
const ipcPort = 43_190
const appPort = 4_396
const bridge = `http://127.0.0.1:${ipcPort}`
const appUrl = `http://127.0.0.1:${appPort}`
/** Only ever opens the throwaway archive made below. */
const demoPassword = 'pathkeep-demo-archive'

/** Where the demo says things are: a Mac user's own folders. */
const home = '/Users/alex'
const places: [from: string, to: string][] = [
  ['PathKeep.app', '/Applications/PathKeep.app'],
  ['project-root', `${home}/Library/Application Support/com.yi-ting.pathkeep`],
  [
    'browsers/chrome-user-data',
    `${home}/Library/Application Support/Google/Chrome`,
  ],
  ['browsers/firefox', `${home}/Library/Application Support/Firefox`],
  ['no-safari', `${home}/Library/Safari`],
  ['os-sandbox', `${home}/Library`],
]
const words: [from: string, to: string][] = [
  ['com.yi-ting.pathkeep.demo-record.backup', 'com.yi-ting.pathkeep.backup'],
  ['fixture@example.test', 'alex@example.com'],
  ['work@example.test', 'alex@work.example.com'],
]

const children: ChildProcess[] = []
const stop = () => {
  for (const child of children) child.kill('SIGTERM')
}
process.on('exit', stop)
for (const signal of ['SIGINT', 'SIGTERM'] as const)
  process.on(signal, () => {
    stop()
    process.exit(1)
  })

async function invoke<T>(name: string, payload: unknown = {}): Promise<T> {
  const response = await fetch(`${bridge}/commands/${name}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  })
  const body = (await response.json()) as T
  if (!response.ok) throw new Error(`${name}: ${JSON.stringify(body)}`)
  return body
}

async function until<T>(
  what: string,
  check: () => Promise<T | null>,
  seconds = 120,
): Promise<T> {
  for (let i = 0; i < seconds * 2; i += 1) {
    try {
      const value = await check()
      if (value !== null) return value
    } catch {
      // Not up yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
  throw new Error(`Gave up waiting for ${what}.`)
}

// 1. Synthetic browsers, and the app's two programs copied in, all under `root`.
rmSync(root, { recursive: true, force: true })
const dir = (name: string) => {
  const at = path.join(root, name)
  mkdirSync(at, { recursive: true })
  return at
}
const programs = dir('PathKeep.app/Contents/MacOS')
for (const name of ['pathkeep-desktop', 'pathkeep-worker'])
  copyFileSync(
    path.join(path.dirname(backend), name),
    path.join(programs, name),
    constants.COPYFILE_FICLONE,
  )
const now = Date.now()
const browsers = writeSyntheticBrowsers(dir('browsers'), { now })
console.log(`Wrote ${browsers.counts.total} synthetic visits.`)

children.push(
  spawn(path.join(programs, 'pathkeep-desktop'), [], {
    stdio: 'inherit',
    env: {
      ...process.env,
      CHB_PROJECT_ROOT: dir('project-root'),
      CHB_CHROME_USER_DATA_DIR: browsers.chromeUserDataRoot,
      CHB_FIREFOX_PROFILES_DIR: browsers.firefoxProfilesRoot,
      CHB_SAFARI_ROOT: dir('no-safari'),
      PATHKEEP_PLATFORM_TEST_SANDBOX_DIR: dir('os-sandbox'),
      PATHKEEP_TEST_BLOCK_NETWORK: '1',
      CHB_TEST_KEYRING_DIR: dir('keyring'),
      PATHKEEP_PLATFORM_TEST_SCHEDULE_LABEL:
        'com.yi-ting.pathkeep.demo-record.backup',
      PATHKEEP_ENABLE_DEV_IPC_BRIDGE: '1',
      PATHKEEP_DEV_IPC_PORT: String(ipcPort),
      PATHKEEP_DEV_IPC_ALLOWED_ORIGINS: appUrl,
      TZ: zone,
    },
  }),
)
await until('the backend', async () =>
  (await fetch(`${bridge}/health`)).ok ? true : null,
)

// The demo pairs this checkout's frontend with this backend's answers: both
// must come from the same commit, with the app's code as committed.
{
  const head = spawnSync('git', ['rev-parse', 'HEAD'], {
    cwd: repo,
    encoding: 'utf8',
  }).stdout.trim()
  const changed = spawnSync(
    'git',
    ['status', '--porcelain', '--', 'src', 'src-tauri'],
    {
      cwd: repo,
      encoding: 'utf8',
    },
  ).stdout.trim()
  const built = await invoke<{ gitCommitFull: string }>('app_build_info')
  if (built.gitCommitFull !== head || changed)
    throw new Error(
      `Record from one commit: the backend was built from ${built.gitCommitFull}, this checkout is at ${head}${changed ? ' with changes in src/ or src-tauri/' : ''}.`,
    )
}

// 2. Set up as a new user who accepts onboarding's defaults: every browser,
// encrypted with the key in the keychain, backed up every hour.
type Snapshot = {
  config: Record<string, unknown>
  browserProfiles: { profileId: string; historyExists: boolean }[]
}
const snapshot = await invoke<Snapshot>('app_snapshot')
const selectedProfileIds = snapshot.browserProfiles
  .filter((p) => p.historyExists && /^(chrome|firefox):/.test(p.profileId))
  .map((p) => p.profileId)
await invoke('initialize_archive', {
  config: {
    ...snapshot.config,
    selectedProfileIds,
    dueAfterHours: 1,
    archiveMode: 'Encrypted',
    rememberDatabaseKeyInKeyring: true,
    preferredLanguage: 'system',
  },
  databaseKey: demoPassword,
})
await invoke('keyring_store_database_key', { value: demoPassword })
const plan = await invoke('preview_schedule', {})
await invoke('apply_schedule', { plan })
const report = await invoke<{ run?: { newVisits?: number } }>(
  'run_backup_now',
  { dueOnly: false },
)
console.log(
  `Backed up ${report.run?.newVisits ?? 0} visits from ${selectedProfileIds.length} profiles.`,
)
await until('the search index', async () =>
  (await invoke<{ phase: string }>('search_indexer_state')).phase === 'ready'
    ? true
    : null,
)

// 3. The record build, served where the bridge accepts it.
const app = dir('app')
const built = spawnSync(
  path.join(repo, 'node_modules/.bin/vite'),
  [
    'build',
    '--config',
    'site/demo/vite.config.ts',
    '--mode',
    'record',
    '--outDir',
    app,
    '--logLevel',
    'warn',
  ],
  {
    cwd: repo,
    stdio: 'inherit',
    env: { ...process.env, PATHKEEP_DEMO_BRIDGE: bridge },
  },
)
if (built.status !== 0) throw new Error('The record build failed.')
const types: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
  '.png': 'image/png',
}
const server = createServer((request, response) => {
  const url = new URL(request.url ?? '/', appUrl)
  const file = path.join(
    app,
    url.pathname === '/' ? 'index.html' : url.pathname,
  )
  try {
    const body = readFileSync(file)
    response.writeHead(200, {
      'content-type': types[path.extname(file)] ?? 'application/octet-stream',
    })
    response.end(body)
  } catch {
    response.writeHead(404).end()
  }
}).listen(appPort, '127.0.0.1')

// 4. The walk.
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({
  viewport: { width: 1360, height: 860 },
  timezoneId: zone,
})
page.on('pageerror', (error) => console.error(`[page error] ${error.message}`))
let shot = 0
const missed: string[] = []
await page.goto(`${appUrl}/?lang=en`)
await walk(page, {
  base: `${appUrl}/?lang=en`,
  settled: async (name) => {
    if (!shots) return
    mkdirSync(shots, { recursive: true })
    await page.screenshot({
      path: path.join(
        shots,
        `${String((shot += 1)).padStart(2, '0')}-${name}.png`,
      ),
    })
  },
  missed: (name, error) => {
    missed.push(name)
    console.warn(`Missed ${name}: ${String(error).split('\n')[0]}`)
  },
})
const log = (await page.evaluate(() => globalThis.pathkeepDemoLog)) as RecordLog
await browser.close()
server.close()

// Every page the search index holds, for the searches the walk never typed:
// the demo answers those itself over this list.
async function everyPage(): Promise<SearchHit[]> {
  const request = { query: '.', regex: true, sort: 'newest', pageSize: 5000 }
  const started = await invoke<{ sessionId: string }>('start_search', {
    request,
  })
  const stream = await fetch(
    `${bridge}/searches/${encodeURIComponent(started.sessionId)}/events`,
  )
  const reader = stream.body!.pipeThrough(new TextDecoderStream()).getReader()
  let buffer = ''
  for (;;) {
    const { value, done } = await reader.read()
    if (done)
      throw new Error('The search for every page ended without an answer.')
    buffer += value
    for (
      let at = buffer.indexOf('\n\n');
      at >= 0;
      at = buffer.indexOf('\n\n')
    ) {
      const data = buffer
        .slice(0, at)
        .split('\n')
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).trim())
        .join('\n')
      buffer = buffer.slice(at + 2)
      if (!data) continue
      const event = JSON.parse(data) as {
        type: string
        data: {
          provisional: boolean
          items: SearchHit[]
          count: { value: number; exact: boolean }
        }
      }
      if (event.type === 'error')
        throw new Error(
          `The search for every page failed: ${JSON.stringify(event.data)}`,
        )
      if (event.type !== 'step' || event.data.provisional) continue
      await reader.cancel()
      const { items, count } = event.data
      if (!count.exact || items.length !== count.value)
        throw new Error(
          `The search for every page returned ${items.length} of ${count.value}.`,
        )
      return items
    }
  }
}
const pages = await everyPage()

/** Where a command takes its page cursor, if it pages. */
function withCursor(args: unknown, cursor: string): unknown {
  const copy = structuredClone(args) as Record<string, unknown>
  for (const holder of [copy, copy?.query, copy?.request] as Record<
    string,
    unknown
  >[])
    if (holder && typeof holder === 'object' && 'cursor' in holder) {
      holder.cursor = cursor
      return copy
    }
  return null
}

/** `args` with every string that reads `from` reading `to` instead. */
function swapped(args: unknown, from: string, to: string): unknown {
  return JSON.parse(
    JSON.stringify(args, (_, value: unknown) => (value === from ? to : value)),
  )
}

const seen = new Set(
  log.calls.map((call) => `${call.command} ${JSON.stringify(call.args)}`),
)
/** Asks the backend and keeps the answer, once per command and arguments. */
async function record(command: string, args: unknown): Promise<unknown> {
  const key = `${command} ${JSON.stringify(args)}`
  const known = log.calls.find(
    (call) =>
      call.command === command &&
      seen.has(key) &&
      JSON.stringify(call.args) === JSON.stringify(args),
  )
  if (known) return known.response
  seen.add(key)
  const response = await invoke(command, args)
  log.calls.push({ command, args, status: 200, response })
  return response
}

// Insights' lists, five pages on, so a visitor can scroll on. (History's
// list comes whole, below.)
type Page = { items?: unknown[]; hasNext?: boolean; nextCursor?: string | null }
for (const first of [...log.calls]) {
  if (first.command === 'query_history') continue
  let response = first.response as Page | null
  for (
    let n = 1;
    n < 5 && response?.nextCursor && response.hasNext !== false;
    n += 1
  ) {
    const args = withCursor(first.args, response.nextCursor)
    if (!args) break
    response = (await record(first.command, args)) as Page
  }
}

// History's whole list, newest first: the demo works out any date range,
// browser, site or page further down from it.
type HistoryArgs = { query: Record<string, unknown> }
const plain = log.calls.find((call) => {
  const query = (call.args as HistoryArgs | null)?.query
  return (
    call.command === 'query_history' &&
    query?.sort === 'newest' &&
    !query.cursor &&
    [
      'q',
      'domain',
      'browserKind',
      'profileId',
      'startTimeMs',
      'endTimeMs',
    ].every((key) => query[key] == null)
  )
})
if (!plain) throw new Error('The walk never opened History.')
const visits: HistoryEntry[] = []
for (
  let page = plain.response as HistoryQueryResponse;
  ;
  page = await invoke<HistoryQueryResponse>(
    'query_history',
    withCursor(plain.args, page.nextCursor!),
  )
) {
  visits.push(...page.items)
  if (!page.hasNext || !page.nextCursor) break
}
const packed = packTimeline(visits)
{
  const unpacked = unpackTimeline(packed)
  const at = visits.findIndex(
    (visit, i) => JSON.stringify(unpacked[i]) !== JSON.stringify(visit),
  )
  if (at >= 0)
    throw new Error(
      `The packed History list does not unpack to what the backend sent:\n  sent ${JSON.stringify(visits[at])}\n  read ${JSON.stringify(unpacked[at])}`,
    )
}

// What a row shows when clicked, for every page the archive has.
for (const { visit } of pages)
  for (const command of ['get_url_detail', 'get_url_annotation'])
    await record(command, { url: visit.url })

// Every day's page in Insights, from the first of January of the archive's
// first year to today, and every site's: whichever a visitor opens is there.
const dayKey = (time: number) =>
  new Intl.DateTimeFormat('en-CA', { timeZone: zone }).format(time)
const first = dayKey(visits.at(-1)!.visitTime).slice(0, 4)
const days: string[] = []
for (
  let at = Date.parse(`${first}-01-01T12:00:00Z`);
  dayKey(at) <= dayKey(now);
  at += 86_400_000
)
  days.push(dayKey(at))
const dayShown = log.calls.filter((call) =>
  shardOf(call.command, call.args)?.startsWith('days/'),
)
if (dayShown.length === 0) throw new Error('The walk never opened a day.')
/** The day the walk opened, as its calls name it. */
const walkedDay = JSON.stringify(dayShown[0].args).match(
  /\d{4}-\d{2}-\d{2}/,
)![0]
for (const day of days)
  for (const call of dayShown)
    await record(call.command, swapped(call.args, walkedDay, day))

const siteShown = log.calls.filter((call) =>
  shardOf(call.command, call.args)?.startsWith('site/'),
)
if (siteShown.length === 0) throw new Error('The walk never opened a site.')
const walkedSite = (
  siteShown[0].args as { request: { registrableDomain: string } }
).request.registrableDomain
const sites = new Set<string>()
JSON.stringify(log.calls, (key, value: unknown) => {
  if (key === 'registrableDomain' && typeof value === 'string') sites.add(value)
  return value
})
for (const site of sites)
  for (const call of siteShown)
    await record(call.command, swapped(call.args, walkedSite, site))
stop()

// 5. Into files: the main one, the History list, a file per day and per
// site. Rewrite where things are, check nothing about this computer is
// left, then write them out.
const files = new Map<string, unknown>()
const shards = new Map<string, RecordedCall[]>()
const main: RecordedCall[] = []
for (const call of log.calls) {
  const shard = shardOf(call.command, call.args)
  if (shard) shards.set(shard, [...(shards.get(shard) ?? []), call])
  else main.push(call)
}
files.set('index', {
  recordedAt: new Date(now).toISOString(),
  zone,
  calls: main,
  searches: log.searches,
  pages,
} satisfies Recording)
files.set('timeline', packed)
for (const [shard, calls] of shards) files.set(shard, calls)

const real = (p: string) => [p, p.startsWith('/var/') ? `/private${p}` : p]
const user = userInfo().username
const email = spawnSync('git', ['config', 'user.email'], {
  encoding: 'utf8',
}).stdout.trim()
const telling = [
  homedir(),
  `/Users/${user}`,
  `/home/${user}`,
  hostname(),
  email,
  repo,
  root,
  tmpdir(),
  '/var/folders/',
  '/private/var/',
  ownZone === zone ? '' : ownZone,
].filter((needle) => needle.length > 2)
const texts = new Map<string, string>()
for (const [name, content] of files) {
  let text = JSON.stringify(content)
  for (const [from, to] of places)
    for (const at of real(path.join(root, from))) text = text.replaceAll(at, to)
  for (const [from, to] of words) text = text.replaceAll(from, to)
  const leaks = telling.filter((needle) => text.includes(needle))
  if (leaks.length > 0)
    throw new Error(
      `recording/${name}.json still says where it was made: ${leaks.join(', ')}`,
    )
  texts.set(name, text)
}
rmSync(out, { recursive: true, force: true })
let bytes = 0
for (const [name, text] of texts) {
  const file = path.join(out, `${name}.json`)
  mkdirSync(path.dirname(file), { recursive: true })
  writeFileSync(file, text)
  bytes += text.length
}
console.log(
  `Recorded ${log.calls.length} calls, ${log.searches.length} searches, ${visits.length} visits on ${pages.length} pages, ${days.length} days and ${sites.size} sites: ${texts.size} files, ${(bytes / 1e6).toFixed(1)} MB, in ${out}.`,
)
if (missed.length > 0) console.warn(`Steps not done: ${missed.join(', ')}`)
rmSync(root, { recursive: true, force: true })
process.exit(missed.length > 0 ? 1 : 0)
