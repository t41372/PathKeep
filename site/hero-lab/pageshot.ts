/**
 * Screenshots site pages for review: the first screen at desktop and phone
 * width as a visitor sees it, and the full page at rest. The full page is
 * taken with reduced motion, so every section stands where it settles
 * (nothing waits to scroll into view) and the opening card is framed.
 *
 *   bun site/hero-lab/pageshot.ts <path> [<path>…] [--width 1440]
 */
import { chromium, type Page } from '@playwright/test'
import { join } from 'node:path'

const argv = process.argv.slice(2)
const width = Number(
  argv.includes('--width') ? argv[argv.indexOf('--width') + 1] : 1440,
)
const paths = argv.filter(
  (a, i) => !a.startsWith('--') && argv[i - 1] !== '--width',
)
const out = join(import.meta.dir, 'out', 'pages')
const browser = await chromium.launch({ channel: 'chrome', headless: true })

/** Opens a page with every picture loaded and the retention chart drawn. */
const open = async (
  path: string,
  options: Parameters<typeof browser.newPage>[0],
): Promise<Page> => {
  const page = await browser.newPage(options)
  await page.goto(`http://127.0.0.1:4391${path}`, { waitUntil: 'networkidle' })
  await page.evaluate(async () => {
    await document.fonts.ready
    document.querySelector('.retention')?.classList.add('is-in')
    for (const img of document.querySelectorAll('img')) img.loading = 'eager'
    await Promise.all(
      Array.from(document.images).map((i) => i.decode().catch(() => undefined)),
    )
  })
  await page.waitForTimeout(1400)
  return page
}

for (const p of paths) {
  const name = p.replace(/^\/|\/$/g, '').replace(/\//g, '-') || 'index'
  const first = await open(p, {
    viewport: { width, height: 900 },
    deviceScaleFactor: 1,
  })
  await first.screenshot({
    path: join(out, `${name}-${width}-top.jpg`),
    type: 'jpeg',
    quality: 86,
  })
  await first.close()

  const still = await open(p, {
    viewport: { width, height: 900 },
    deviceScaleFactor: 1,
    reducedMotion: 'reduce',
  })
  await still.screenshot({
    path: join(out, `${name}-${width}-full.jpg`),
    type: 'jpeg',
    quality: 80,
    fullPage: true,
  })
  await still.close()

  const phone = await open(p, {
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
  })
  await phone.screenshot({
    path: join(out, `${name}-phone.jpg`),
    type: 'jpeg',
    quality: 80,
  })
  await phone.close()
  console.log(name)
}
await browser.close()
