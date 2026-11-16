/**
 * What a visitor does in the demo, in the order the recording does it: every
 * screen, its tabs, filters and ranges, scrolling to the end of each list,
 * a few searches, and the previews a change starts from (a new backup
 * interval, what would be deleted), closed before anything is applied.
 * Read-only: nothing here changes the archive, because the demo saves
 * nothing.
 *
 * `record.ts` walks it against the real backend to make the recording; the
 * demo's check walks it against the recording, where every command must
 * find its exact answer. Labels are the English catalog's.
 */
import type { Locator, Page } from '@playwright/test'

export interface WalkOptions {
  /** The app's address, up to the `#`. */
  base: string
  /** Called after each step, once the screen has settled. */
  settled?: (name: string) => Promise<void>
  /** A step that could not be done; the walk goes on. */
  missed?: (name: string, error: unknown) => void
}

/** Every screen, by address. */
export const routes = [
  'browsing/home',
  'browsing/history',
  'browsing/insights',
  'browsing/backup',
  'ask',
  ...['general', 'security', 'ai', 'background', 'storage', 'about'].map(
    (s) => `settings/${s}`,
  ),
]

/**
 * Opens every screen by its address, nothing clicked: what the walk can do
 * in any language, since its clicks go by the English labels.
 */
export async function tour(page: Page, { base, settled }: WalkOptions) {
  for (const route of routes) {
    await page.goto(`${base}#/${route}`)
    await page.waitForLoadState('networkidle')
    await page.waitForTimeout(400)
    await settled?.(route.replace('/', '-'))
  }
}

/** Searches a visitor might type; each is recorded with its results. */
export const searches = [
  'tokio',
  'camera',
  'strava',
  'JLPT',
  'sqlite',
  '台北',
  'github issues',
  '/tokio|smol/',
]

export async function walk(page: Page, { base, settled, missed }: WalkOptions) {
  const settle = async (name: string) => {
    await page.waitForLoadState('networkidle')
    await page.waitForTimeout(400)
    await settled?.(name)
  }
  const step = async (name: string, action: () => Promise<void>) => {
    try {
      await action()
      await settle(name)
    } catch (error) {
      missed?.(name, error)
    }
  }
  const go = async (route: string) => {
    await page.goto(`${base}#/${route}`)
  }
  /**
   * Scrolls the screen to its end, a screenful at a time: the largest area
   * in it that scrolls (a list scrolls inside the screen, not the screen).
   */
  const scrollThrough = async (screen: Locator) => {
    for (let i = 0; i < 40; i += 1) {
      const done = await screen.evaluate((root) => {
        const areas = [root, ...root.querySelectorAll<HTMLElement>('*')].filter(
          (el) =>
            el.scrollHeight > el.clientHeight + 8 &&
            /auto|scroll/.test(getComputedStyle(el).overflowY),
        )
        const area = areas.sort((a, b) => b.clientHeight - a.clientHeight)[0]
        if (!area) return true
        area.scrollBy(0, area.clientHeight * 0.9)
        return area.scrollTop + area.clientHeight >= area.scrollHeight - 2
      })
      await page.waitForTimeout(300)
      await page.waitForLoadState('networkidle')
      if (done) break
    }
  }
  const main = page.locator('main').first()
  const pick = async (control: string, option: string) => {
    await page
      .getByRole('button', { name: control })
      .or(page.getByRole('combobox', { name: control }))
      .first()
      .click()
    await page
      .getByRole('option', { name: option })
      .or(page.getByRole('menuitemradio', { name: option }))
      .or(page.getByRole('menuitem', { name: option }))
      .first()
      .click()
  }

  // Home
  await step('home', () => go('browsing/home'))
  await step('home-scrolled', () => scrollThrough(main))
  await step('home-previous-year', () =>
    page.getByRole('button', { name: 'Previous year' }).click(),
  )
  await step('home-next-year', () =>
    page.getByRole('button', { name: 'Next year' }).click(),
  )
  // A day in the year (the last one with visits): its page in Insights.
  await step('home-day', async () => {
    await go('browsing/home')
    await page.waitForLoadState('networkidle')
    await page.locator('[data-cell][role="button"]').last().click()
    await page.waitForLoadState('networkidle')
    await scrollThrough(main)
  })

  // Each page in "Things you keep coming back to": its page in Insights.
  await go('browsing/home')
  await page.waitForLoadState('networkidle')
  await scrollThrough(main)
  const pageLinks = main.locator('a[href*="/insights/page/"]')
  await pageLinks
    .first()
    .waitFor()
    .catch((error) => missed?.('home-returns-to', error))
  const returnsTo = await pageLinks.evaluateAll((links) =>
    links.map((link) => link.getAttribute('href')!),
  )
  for (const href of returnsTo)
    await step(`home-returns-to-${returnsTo.indexOf(href)}`, async () => {
      await page.goto(`${base}${href}`)
      await page.waitForLoadState('networkidle')
      await scrollThrough(main)
    })

  // History
  await step('history', () => go('browsing/history'))
  // The first visit listed, whichever it is: the synthetic history moves
  // with the day it is recorded on.
  await step('history-row', () =>
    page
      .getByRole('listbox', { name: 'History timeline' })
      .getByRole('option')
      .first()
      .click(),
  )
  await step('history-row-closed', () => page.keyboard.press('Escape'))
  await step('history-scrolled', () => scrollThrough(main))
  for (const range of [
    'Today',
    'Yesterday',
    'Last 7 days',
    'Last 30 days',
    'All dates',
  ])
    await step(`history-${range}`, () => pick('Date filter', range))
  for (const view of ['Sites', 'Starred', 'Timeline'])
    await step(`history-${view}`, async () => {
      await page
        .getByRole('tab', { name: view })
        .or(page.getByRole('radio', { name: view }))
        .first()
        .click()
      await page.waitForLoadState('networkidle')
      await scrollThrough(main)
    })
  const box = page
    .getByRole('searchbox', { name: 'Search history' })
    .or(page.getByPlaceholder(/Search titles/))
    .first()
  for (const query of searches)
    await step(`search-${query}`, async () => {
      await box.fill(query)
      await page.waitForTimeout(800)
      await page.waitForLoadState('networkidle')
    })
  await step('search-cleared', () => box.fill(''))

  // Insights
  await step('insights', () => go('browsing/insights'))
  await step('insights-scrolled', () => scrollThrough(main))
  for (const range of ['1y', '90d', '30d', '7d', 'All time'])
    await step(`insights-${range}`, async () => {
      await page
        .getByRole('radio', { name: range })
        .or(page.getByRole('button', { name: range, exact: true }))
        .first()
        .click()
      await page.waitForLoadState('networkidle')
      await scrollThrough(main)
    })

  // A site from the top sites: its page in Insights.
  await step('insights-site', async () => {
    await go('browsing/insights')
    await page.waitForLoadState('networkidle')
    await page
      .locator('main')
      .getByText('strava.com', { exact: true })
      .first()
      .click()
    await page.waitForLoadState('networkidle')
    await scrollThrough(main)
  })

  // Backup, and the preview each other backup interval starts from
  await step('backup', () => go('browsing/backup'))
  await step('backup-scrolled', () => scrollThrough(main))
  for (const interval of ['Every 6h', 'Daily', 'Off'])
    await step(`backup-${interval}`, async () => {
      await go('browsing/backup')
      await page.waitForLoadState('networkidle')
      await page.getByRole('radio', { name: interval }).click()
      await page.getByRole('dialog').waitFor()
      await page.waitForLoadState('networkidle')
      await page.keyboard.press('Escape')
    })

  // Ask, Settings
  await step('ask', () => go('ask'))
  for (const section of [
    'general',
    'security',
    'ai',
    'background',
    'storage',
    'about',
  ])
    await step(`settings-${section}`, async () => {
      await go(`settings/${section}`)
      await page.waitForLoadState('networkidle')
      await scrollThrough(main)
    })
  // What keeping less history would delete, and the archive's health check.
  await step('settings-storage-review', async () => {
    await go('settings/storage')
    await page.waitForLoadState('networkidle')
    await page.getByRole('button', { name: 'Review…' }).click()
    await page.getByRole('dialog').waitFor()
    await page.waitForLoadState('networkidle')
    await page.keyboard.press('Escape')
  })
  await step('settings-about-check', async () => {
    await go('settings/about')
    await page.waitForLoadState('networkidle')
    await page.getByRole('button', { name: 'Check', exact: true }).click()
    await page.waitForTimeout(600)
    await page.waitForLoadState('networkidle')
  })

  // The command palette
  await step('palette', async () => {
    await go('browsing/home')
    await page.waitForLoadState('networkidle')
    await page.keyboard.press('Meta+K')
    await page.keyboard.type('git', { delay: 60 })
    await page.waitForTimeout(800)
  })
  await page.keyboard.press('Escape')
}
