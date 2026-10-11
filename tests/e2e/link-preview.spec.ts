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
 * The site is a local HTTPS server (`support/preview-server.ts`); the visits
 * to it go into Chrome's history and are backed up like any other browsing.
 */
import { expect, test, type Page } from '@playwright/test'
import { appendChromeVisits } from '../../scripts/fixtures/synthetic-browsers.mjs'
import { fixture } from './support/fixture'
import {
  PREVIEW_VISIT_COUNT,
  startPreviewServer,
} from './support/preview-server'

const ARTICLE_TITLE = 'Preview harbor article'
const DETOUR_TITLE = 'Preview detour article'
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
  await page.goto('/#/history')
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
    expect(visits).toHaveLength(PREVIEW_VISIT_COUNT)
    appendChromeVisits(fixture().files['chrome:Default'], visits)
    await page.goto('/#/backup')
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

function visit(url: string, title: string, at: number) {
  return { url, title, at, typed: false, duration: 30_000 }
}
