/**
 * Cuts the pictures for the feature wall (`src/components/Features.astro`),
 * once per language: the browsing ones from the demo, which is the real app
 * in that language on the recorded archive, so their words are the app's
 * own; the Agent one from the Agent insights screen `capture-screens.ts`
 * took. Needs the preview server, with the demo built into it.
 *
 *   bun site/hero-lab/capture-features.ts
 *
 * Writes `hero-lab/screens-in/<slug>/feature-<id>.png` at twice the pixels;
 * then run `export-screens.ts` for the WebP files the page loads.
 */
import { chromium, type Page } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { langs } from '../src/content/copy'

const out = join(import.meta.dir, 'screens-in')
const demo = 'http://127.0.0.1:4391/app/'

/** A cut of a 1360×860 screen of the demo, and what to do there first. */
const cuts: {
  id: string
  route: string
  clip: { x: number; y: number; width: number; height: number }
  before?: (page: Page) => Promise<void>
}[] = [
  {
    id: 'search',
    route: 'browsing/history',
    clip: { x: 96, y: 18, width: 1240, height: 450 },
    before: async (page) => {
      // A pattern: the search box takes /regex/ as well as words.
      await page.locator('main input').first().fill('/tokio|smol/')
      await page.waitForTimeout(1500)
    },
  },
  {
    id: 'history',
    route: 'browsing/history',
    clip: { x: 96, y: 136, width: 760, height: 550 },
  },
  {
    id: 'insights',
    route: 'browsing/insights',
    clip: { x: 200, y: 30, width: 1020, height: 550 },
  },
]

/** The Agent one, from the Agent insights screen, in its pixels. */
const agent = { x: 200, y: 190, width: 2440, height: 1070 }

const browser = await chromium.launch({ channel: 'chrome', headless: true })
try {
  for (const { lang, slug } of langs) {
    const page = await browser.newPage({
      viewport: { width: 1360, height: 860 },
      deviceScaleFactor: 2,
    })
    for (const cut of cuts) {
      await page.goto(`${demo}?lang=${lang}#/${cut.route}`)
      await page.locator('main').first().waitFor()
      await page.waitForLoadState('networkidle')
      await page.waitForTimeout(1500)
      await cut.before?.(page)
      const file = join(out, slug, `feature-${cut.id}.png`)
      await page.screenshot({ path: file, clip: cut.clip })
      console.log(`screens-in/${slug}/feature-${cut.id}.png`)
    }
    await page.close()

    const screen = await readFile(join(out, slug, 'agent-insights.png'))
    const sheet = await browser.newPage({
      viewport: { width: 2720, height: 1720 },
    })
    await sheet.setContent(
      `<body style="margin:0"><img src="data:image/png;base64,${screen.toString('base64')}"></body>`,
    )
    await sheet.locator('img').evaluate((img: HTMLImageElement) => img.decode())
    await sheet.screenshot({
      path: join(out, slug, 'feature-agent.png'),
      clip: agent,
    })
    await sheet.close()
    console.log(`screens-in/${slug}/feature-agent.png`)
  }
} finally {
  await browser.close()
}
