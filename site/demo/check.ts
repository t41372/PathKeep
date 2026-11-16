/**
 * Checks a built demo against its recording: walks `walk.ts` in Chrome (in
 * English; other languages tour every screen by address) and fails if the
 * app sent anything the recording has no answer for (exact, or worked out
 * in the page from what it holds), or if the page threw. In English it also
 * makes the changes a visitor would (a backup, the schedule, a star, a
 * note, a setting) and fails if the app shows an error for one, then opens
 * Insights again, whose cards read their own sections once the backup has
 * made their copies stale. In another
 * time zone the History date presets are worked out in the page, and lists
 * the app asks for up to the visitor's own today take the recorded answer
 * with the closest dates; both are reported, not failed. There it fails if
 * Insights offer to move to the visitor's zone.
 * Run after `vite build --config site/demo/vite.config.ts`.
 *
 *   node site/demo/check.ts [--dir <built demo>] [--lang en] [--zone <IANA zone>] [--shots <dir>]
 */
import { mkdirSync, readdirSync, readFileSync } from 'node:fs'
import { createServer } from 'node:http'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from '@playwright/test'
import { tour, walk } from './walk.ts'

const here = path.dirname(fileURLToPath(import.meta.url))
const argv = process.argv.slice(2)
const flag = (name: string) =>
  argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined
const dir = path.resolve(flag('--dir') ?? path.join(here, '../public/app'))
const lang = flag('--lang') ?? 'en'
const shots = flag('--shots')
const recordedIn = (
  JSON.parse(readFileSync(path.join(dir, 'recording/index.json'), 'utf8')) as {
    zone: string
  }
).zone
/** Where the visitor is; the recording's own zone unless told. */
const zone = flag('--zone') ?? recordedIn
const elsewhere = zone !== recordedIn
const port = 4_397
const base = `http://127.0.0.1:${port}`

const types: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
}
const server = createServer((request, response) => {
  const url = new URL(request.url ?? '/', base)
  const file = path.join(
    dir,
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
}).listen(port, '127.0.0.1')

const problems: string[] = []
const workedOut: string[] = []
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({
  viewport: { width: 1360, height: 860 },
  timezoneId: zone,
})
page.on('pageerror', (error) => problems.push(`page error: ${error.message}`))
page.on('console', (message) => {
  const text = message.text()
  if (
    /^\[demo\] ((listed|searched) in the page|taken from its batch)/.test(text)
  )
    workedOut.push(text)
  else if (elsewhere && text.startsWith('[demo] nearest answer'))
    workedOut.push(text)
  else if (text.startsWith('[demo]')) problems.push(text)
})
page.setDefaultTimeout(10_000)
let shot = 0
await page.goto(`${base}/?lang=${lang}`)
try {
  await page.locator('main').first().waitFor({ timeout: 30_000 })
} catch {
  console.error(
    `The demo did not start (${problems.join('; ') || 'no screen drawn'}). Is ${path.join(dir, 'recording/index.json')} there?`,
  )
  process.exit(1)
}
await (lang === 'en' ? walk : tour)(page, {
  base: `${base}/?lang=${lang}`,
  settled: async (name) => {
    if (!shots) return
    mkdirSync(shots, { recursive: true })
    const file = `${String((shot += 1)).padStart(2, '0')}-${name.replace(/[^\w-]+/g, '_')}.png`
    await page.screenshot({ path: path.join(shots, file) })
  },
  missed: (name, error) =>
    problems.push(`step ${name}: ${String(error).split('\n')[0]}`),
})
// A search the walk never typed: the demo searches the recorded pages itself.
if (lang === 'en') {
  const box = page.getByPlaceholder(/Search titles/).first()
  await page.goto(`${base}/?lang=en#/browsing/history`)
  await box.fill('marathon taper')
  await page.waitForTimeout(800)
  const found = await page
    .locator('main')
    .getByText(/marathon taper/i)
    .count()
  if (found === 0) problems.push('a search outside the recording found nothing')
  if (shots)
    await page.screenshot({
      path: path.join(shots, 'search-outside-the-recording.png'),
    })

  // Locking, never recorded: without a passcode the app says one comes
  // first and opens its settings, as over the real backend.
  await page.goto(`${base}/?lang=en#/browsing/home`)
  await page.getByRole('button', { name: /Lock PathKeep/ }).click()
  await page
    .waitForURL(/#\/settings\/security/, { timeout: 5_000 })
    .catch(() => problems.push('locking did not lead to the passcode settings'))

  // Changes go through on screen with no error, and are kept nowhere
  // (writes.ts): a backup, the backup schedule, a star, a note, a setting.
  const change = async (
    name: string,
    act: () => Promise<void>,
    says?: RegExp,
  ) => {
    try {
      await act()
      if (says) await page.getByText(says).first().waitFor({ timeout: 5_000 })
      await page.waitForTimeout(800)
      const shown = await page
        .locator('[data-sonner-toast][data-type="error"], main [role="alert"]')
        .count()
      if (shown > 0) problems.push(`${name}: the app showed an error`)
    } catch (error) {
      problems.push(`${name}: ${String(error).split('\n')[0]}`)
    }
  }
  await page.goto(`${base}/?lang=en#/browsing/home`)
  await change(
    'back up now',
    () => page.getByRole('button', { name: 'Back up now' }).first().click(),
    /no new visits/,
  )
  await page.goto(`${base}/?lang=en#/browsing/backup`)
  await change('the backup schedule', async () => {
    await page.getByRole('radio', { name: 'Every 6h' }).click()
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Update' })
      .click()
  })
  await page.goto(`${base}/?lang=en#/browsing/history`)
  await page
    .getByRole('listbox', { name: 'History timeline' })
    .getByRole('option')
    .first()
    .click()
  await change(
    'a star',
    () => page.getByRole('button', { name: 'Star this page' }).click(),
    /^Starred$/,
  )
  await change(
    'a note',
    () =>
      page
        .getByPlaceholder('Add a note you can search later')
        .fill('Taper week starts Monday'),
    /^Saved$/,
  )
  await page.goto(`${base}/?lang=en#/settings`)
  await change('a setting', () =>
    page
      .getByRole('switch', { name: /Open at login/ })
      .first()
      .click(),
  )

  // Insights again, after the backup made every card's copy stale: the
  // cards read their own sections now (backend.ts takes them from their
  // batch). A visitor in another time zone is not asked to move Insights
  // to it: the archive's owner kept them in its own.
  await page.goto(`${base}/?lang=en#/browsing/insights`)
  await page.locator('main').first().waitFor()
  await page.waitForLoadState('networkidle')
  await page.waitForTimeout(1_000)
  if (
    elsewhere &&
    (await page.getByRole('button', { name: /^Keep / }).count()) > 0
  )
    problems.push('Insights offer to move to the visitor’s time zone')

  // Pages the walk opens only one of: a day (the first, a middle and the
  // last month recorded), every site, and History narrowed to a site and a
  // day long ago. Each must come from its own file or the whole History list.
  const months = readdirSync(path.join(dir, 'recording/days')).sort()
  const dayIn = (month: string) => {
    const calls = JSON.parse(
      readFileSync(path.join(dir, 'recording/days', month), 'utf8'),
    ) as { args: unknown }[]
    return JSON.stringify(calls[0].args).match(/\d{4}-\d{2}-\d{2}/)![0]
  }
  const days = [
    months[0],
    months[Math.floor(months.length / 2)],
    months.at(-1)!,
  ].map(dayIn)
  const sites = readdirSync(path.join(dir, 'recording/site')).map((file) =>
    decodeURIComponent(file.replace(/\.json$/, '')),
  )
  const opens = [
    ...days.map((day) => `browsing/insights/day/${day}`),
    ...sites.map(
      (site) => `browsing/insights/site/${encodeURIComponent(site)}`,
    ),
    `browsing/history?domain=${encodeURIComponent(sites[0])}`,
    `browsing/history?date=${days[1]}`,
  ]
  for (const route of opens) {
    await page.goto(`${base}/?lang=en#/${route}`)
    await page.waitForLoadState('networkidle')
    await page.waitForTimeout(300)
    const empty = await page
      .locator('main')
      .getByText(/Couldn’t load/)
      .count()
    if (empty > 0) problems.push(`${route} could not load something`)
  }
}
await browser.close()
server.close()

if (problems.length > 0) {
  console.error(`The demo strayed from its recording ${problems.length} times:`)
  for (const problem of problems) console.error(`  ${problem}`)
  process.exit(1)
}
console.log(
  `The demo answered every step of the walk from its recording${workedOut.length > 0 ? ` (${workedOut.length} worked out in the page or from the closest dates)` : ''}.`,
)
