/**
 * Link previews downloaded for real.
 *
 * Proves: a backed-up page's og:image is fetched from its site (the page and
 * the image once each, whether History's rows or the details ask first),
 * drawn at its own size and kept: after a reload the site is asked for
 * nothing more. An og:image that redirects to a private address
 * (169.254.169.254, where cloud machines keep their credentials) is refused
 * at the redirect, and the details say no preview could be had.
 *
 * The switch and its two options are settings of browsing history
 * (`data-spaces.md` §8): with previews off, a page backed up meanwhile says
 * so and asks nothing of its site, and its "Turn on" leads to Settings ›
 * Browsing history. There the options open under the switch, are saved as
 * chosen and survive a reload, and "When shown" fetches the page's image
 * once it is shown. General keeps one row that points there. The preview
 * images kept and Export history are on that page too, not in Storage, and
 * the images are deleted only after a question that says how many.
 *
 * The site is a local HTTPS server (`support/preview-server.ts`); the visits
 * to it go into Chrome's history and are backed up like any other browsing.
 */
import { expect, test, type Page } from '@playwright/test'
import { appendChromeVisits } from '../../scripts/fixtures/synthetic-browsers.mjs'
import type { AppSnapshot, OgImageStorageStats } from '../../src/lib/types'
import { unlockIfLocked } from './support/app'
import { backend, fixture } from './support/fixture'
import {
  PREVIEW_VISIT_COUNT,
  startPreviewServer,
} from './support/preview-server'

const ARTICLE_TITLE = 'Preview harbor article'
const DETOUR_TITLE = 'Preview detour article'
const QUIET_TITLE = 'Preview quiet article'
/** Visits each test adds; `wipe.spec` counts them all. */
const FETCHED_VISITS = 2
const QUIET_VISITS = 1
/** A 40×21 PNG. */
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAACgAAAAVCAIAAAC7eDtJAAAAJUlEQVR4nGP4UKExIIhh1OJRi0ctHrV41OJRi0ctHrV41GKaIwDjSCDM9x1XGwAAAABJRU5ErkJggg==',
  'base64',
)

/** An HTML page titled `title` whose og:image is `image` on the same site. */
const htmlPage = (title: string, image: string) => (origin: string) => ({
  headers: { 'content-type': 'text/html; charset=utf-8' },
  body: `<!doctype html><html><head><title>${title}</title><meta property="og:image" content="${origin}${image}"></head><body>${title}</body></html>`,
})

/** Finds the page in History, opens its details and returns the link preview. */
async function openDetails(page: Page, title: string) {
  await page.goto('/#/browsing/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill(title)
  const row = page
    .getByRole('listbox', { name: 'History results' })
    .getByRole('option')
    .filter({ hasText: title })
  await expect(row).toHaveCount(1, { timeout: 30_000 })
  await row.click()
  return page
    .getByRole('complementary', { name: 'Page details' })
    .getByRole('figure', { name: 'Link preview' })
}

test('a page’s preview image is downloaded once; a redirect to a private address is refused', async ({
  page,
}) => {
  const site = await startPreviewServer({
    '/article': htmlPage(ARTICLE_TITLE, '/cover.png'),
    '/cover.png': () => ({
      headers: { 'content-type': 'image/png' },
      body: PNG,
    }),
    '/detour': htmlPage(DETOUR_TITLE, '/to-metadata'),
    '/to-metadata': () => ({
      status: 302,
      headers: { location: 'http://169.254.169.254/latest/meta-data/' },
    }),
  })
  try {
    // A minute apart, an hour ago.
    const at = Date.now() - 3_600_000
    const visits = [
      visit(`${site.origin}/article`, ARTICLE_TITLE, at),
      visit(`${site.origin}/detour`, DETOUR_TITLE, at + 60_000),
    ]
    expect(visits).toHaveLength(FETCHED_VISITS)
    expect(FETCHED_VISITS + QUIET_VISITS).toBe(PREVIEW_VISIT_COUNT)
    appendChromeVisits(fixture().files['chrome:Default'], visits)
    await page.goto('/#/browsing/backup')
    await page.getByRole('button', { name: 'Back up now' }).first().click()
    await expect(
      page
        .getByRole('heading', { name: 'Recent runs' })
        .locator('xpath=ancestor::*[.//li][1]')
        .getByRole('listitem')
        .first(),
    ).toContainText(new RegExp(`\\b${visits.length} new visits`), {
      timeout: 60_000,
    })

    let figure = await openDetails(page, ARTICLE_TITLE)
    const image = figure.getByRole('img', {
      name: `Preview image of ${ARTICLE_TITLE}`,
    })
    await expect(image).toBeVisible({ timeout: 30_000 })
    await expect(figure).toHaveAttribute('data-preview-state', 'ok')
    expect(
      await image.evaluate((img: HTMLImageElement) => img.naturalWidth),
    ).toBe(40)
    // Each asked for once, whichever part of History asked first.
    const asked = (path: string) =>
      site.requests.filter((request) => request === path).length
    expect(asked('/article')).toBe(1)
    expect(asked('/cover.png')).toBe(1)

    // Kept: opening the page again shows it without asking the site.
    await page.reload()
    figure = await openDetails(page, ARTICLE_TITLE)
    await expect(figure).toHaveAttribute('data-preview-state', 'ok')
    expect(asked('/article')).toBe(1)
    expect(asked('/cover.png')).toBe(1)

    // The image's address answers with a redirect to a private address: the
    // redirect is where it stops.
    figure = await openDetails(page, DETOUR_TITLE)
    await expect(figure).toHaveAttribute('data-preview-state', 'failed', {
      timeout: 30_000,
    })
    await expect(figure).toContainText('Couldn’t get a preview image')
    await expect(figure.getByRole('img')).toHaveCount(0)
    expect(asked('/detour')).toBe(1)
    expect(asked('/to-metadata')).toBe(1)
    // Nothing else was asked of the site: no path retried, nothing past the
    // redirect.
    expect(new Set(site.requests)).toEqual(
      new Set(['/article', '/cover.png', '/detour', '/to-metadata']),
    )
  } finally {
    await site.close()
  }
})

test('link previews are set in Settings › Browsing history, where History’s “Turn on” leads', async ({
  page,
}) => {
  const before = (await backend<AppSnapshot>('app_snapshot')).config.ogImage!
  const site = await startPreviewServer({
    '/quiet': htmlPage(QUIET_TITLE, '/quiet.png'),
    '/quiet.png': () => ({
      headers: { 'content-type': 'image/png' },
      body: PNG,
    }),
  })
  const asked = (path: string) =>
    site.requests.filter((request) => request === path).length
  const ogImage = async () =>
    (await backend<AppSnapshot>('app_snapshot')).config.ogImage!
  const previews = page.getByRole('switch', { name: 'Link previews' })
  const options = page.getByRole('group', { name: 'Link preview options' })
  const when = options.getByRole('combobox', { name: 'When to fetch' })
  const keep = options.getByRole('combobox', { name: 'Keep images' })
  try {
    await page.goto('/#/settings/browsing')
    await unlockIfLocked(page)
    await expect(
      page
        .getByRole('navigation', { name: 'Settings sections' })
        .getByRole('group', { name: 'Data types' })
        .getByRole('link', { name: 'Browsing history' }),
    ).toHaveAttribute('aria-current', 'page')
    await expect(
      page.getByRole('link', { name: 'Open the backup page' }),
    ).toHaveAttribute('href', '#/browsing/backup')

    // On, as the test above needed: the options show what is saved.
    expect(before.fetchEnabled && before.fetchMode !== 'off').toBe(true)
    await expect(previews).toBeChecked()
    await expect(when).toHaveText(FETCH_MODE_LABEL[before.fetchMode])
    await expect(keep).toHaveText(keepLabel(before.cleanup))

    // Off: the options close, and the setting is saved.
    await previews.click()
    await expect(previews).not.toBeChecked()
    await expect(options).toHaveCount(0)
    await expect.poll(async () => (await ogImage()).fetchEnabled).toBe(false)

    // A page backed up while they are off is never asked for its image.
    appendChromeVisits(fixture().files['chrome:Default'], [
      visit(`${site.origin}/quiet`, QUIET_TITLE, Date.now() - 1_800_000),
    ])
    await page.goto('/#/browsing/backup')
    await page.getByRole('button', { name: 'Back up now' }).first().click()
    await expect(
      page
        .getByRole('heading', { name: 'Recent runs' })
        .locator('xpath=ancestor::*[.//li][1]')
        .getByRole('listitem')
        .first(),
    ).toContainText(new RegExp(`\\b${QUIET_VISITS} new visits?\\b`), {
      timeout: 60_000,
    })
    let figure = await openDetails(page, QUIET_TITLE)
    await expect(figure).toHaveAttribute('data-preview-state', 'off')
    await expect(figure).toContainText('Link previews are off')
    expect(site.requests).toEqual([])

    // "Turn on" goes to the switch, and the options open under it.
    await figure.getByRole('link', { name: 'Turn on' }).click()
    await expect(page).toHaveURL(/#\/settings\/browsing$/)
    await previews.click()
    await expect(previews).toBeChecked()
    await expect(options).toBeVisible()
    await when.click()
    await page.getByRole('option', { name: 'When shown' }).click()
    await expect(when).toHaveText('When shown')
    await keep.click()
    await page.getByRole('option', { name: 'For 90 days' }).click()
    await expect(keep).toHaveText('For 90 days')
    await expect
      .poll(async () => {
        const saved = await ogImage()
        return {
          enabled: saved.fetchEnabled,
          mode: saved.fetchMode,
          cleanup: saved.cleanup,
        }
      })
      .toEqual({
        enabled: true,
        mode: 'on_demand',
        cleanup: { mode: 'timeTtl', maxAgeDays: 90 },
      })
    await page.reload()
    await unlockIfLocked(page)
    await expect(when).toHaveText('When shown')
    await expect(keep).toHaveText('For 90 days')

    // "When shown": showing the page fetches its image, once.
    figure = await openDetails(page, QUIET_TITLE)
    await expect(
      figure.getByRole('img', { name: `Preview image of ${QUIET_TITLE}` }),
    ).toBeVisible({ timeout: 30_000 })
    await expect(figure).toHaveAttribute('data-preview-state', 'ok')
    expect(asked('/quiet')).toBe(1)
    expect(asked('/quiet.png')).toBe(1)

    // General has no switches of its own now, only the way here.
    await page.goto('/#/settings/general')
    await unlockIfLocked(page)
    await expect(
      page.getByRole('switch', { name: 'Link previews' }),
    ).toHaveCount(0)
    await expect(
      page.getByRole('switch', { name: 'Page summaries' }),
    ).toHaveCount(0)
    const online = page
      .getByText('Online', { exact: true })
      .locator('xpath=../..')
    await expect(online).toContainText(
      'Only link previews and page summaries, in the settings of Browsing history, contact websites.',
    )
    await online
      .getByRole('link', { name: 'Browsing history settings' })
      .click()
    await expect(page).toHaveURL(/#\/settings\/browsing$/)
    await expect(
      page.getByRole('switch', { name: 'Page summaries' }),
    ).toBeVisible()

    // The images kept are browsing history's too: counted here, and deleted
    // only after a question that says how many.
    const storedImages = async () =>
      (await backend<OgImageStorageStats>('get_og_image_storage_stats'))
        .blobCount
    const kept = await storedImages()
    expect(kept).toBeGreaterThanOrEqual(1)
    const images = page
      .getByText('Link preview images', { exact: true })
      .locator('xpath=../..')
    const imageCount = new RegExp(`\\b${kept} images?,`)
    await expect(images).toContainText(imageCount)
    await images.getByRole('button', { name: 'Clear…' }).click()
    const asking = page.getByRole('alertdialog', {
      name: 'Delete all link preview images?',
    })
    await expect(asking).toContainText(new RegExp(`\\b${kept} images? \\(`))
    await asking.getByRole('button', { name: 'Clear…' }).click()
    await expect.poll(storedImages).toBe(0)
    await expect(images.getByRole('button', { name: 'Clear…' })).toBeDisabled()
    await expect(
      page.getByText('Export history', { exact: true }),
    ).toBeVisible()

    // Storage keeps neither of them.
    await page.goto('/#/settings/storage')
    await expect(
      page.getByRole('heading', { name: 'Storage', level: 2 }),
    ).toBeVisible()
    await expect(
      page.getByText('Link preview images', { exact: true }),
    ).toHaveCount(0)
    await expect(page.getByText('Export history', { exact: true })).toHaveCount(
      0,
    )
  } finally {
    await site.close()
    const now = await backend<AppSnapshot>('app_snapshot')
    await backend('save_config', { config: { ...now.config, ogImage: before } })
  }
})

const FETCH_MODE_LABEL: Record<string, string> = {
  on_demand: 'When shown',
  background: 'After each backup',
}

/** The "Keep images" label for a saved cleanup rule (`link-preview-keep.ts`). */
function keepLabel(
  cleanup: NonNullable<AppSnapshot['config']['ogImage']>['cleanup'],
) {
  const MB = 1024 * 1024
  if (cleanup.mode === 'off') return 'Until cleared'
  if (cleanup.mode === 'timeTtl' && cleanup.maxAgeDays === 30)
    return 'For 30 days'
  if (cleanup.mode === 'timeTtl' && cleanup.maxAgeDays === 90)
    return 'For 90 days'
  if (cleanup.mode === 'lru' && cleanup.maxBytes === 100 * MB)
    return 'Up to 100 MB'
  if (cleanup.mode === 'lru' && cleanup.maxBytes === 500 * MB)
    return 'Up to 500 MB'
  return 'Custom'
}

function visit(url: string, title: string, at: number) {
  return { url, title, at, typed: false, duration: 30_000 }
}
