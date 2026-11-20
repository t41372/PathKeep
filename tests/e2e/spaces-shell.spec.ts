/**
 * The shell that holds one space per kind of data (`WORK-SPACES-SHELL`),
 * with the one space the app has: browsing history.
 *
 * Proves, on the archive the first backup built:
 * - `#/` opens the space's first page, `#/browsing/home`. The switcher
 *   names the space; its menu lists that one space with the archive's visit
 *   count and the number of browsers with a profile backed up, as the
 *   backend counts them. Agent conversations are off, so the menu's one
 *   other item offers to keep another kind of data, and it opens Settings ›
 *   Manage data types. Each page in the rail
 *   opens its `#/browsing/…` address, and on Settings and Ask the rail keeps
 *   the space's pages.
 * - The rail has no language control. Choosing a language in Settings ›
 *   General renames the rail's pages (History is 紀錄 in Traditional
 *   Chinese), and ⌘K, in that language, lists the page as
 *   `瀏覽紀錄 · 紀錄` and searches the space for typed words on its list
 *   page.
 * - ⌘K in English: the space's pages under Go to, and the row that searches
 *   the space opens History with the words in its search box.
 * - From Settings, ⌘1 (Ctrl+1 off macOS) opens the space's first page.
 *
 * Switching between two spaces (the slide, ⌘2, the "Switch to" group) waits
 * for a second space to exist; there is no stand-in space to test it with.
 */
import { expect, test, type Page } from '@playwright/test'
import type { AppSnapshot } from '../../src/lib/types'
import { unlockIfLocked } from './support/app'
import { archivedVisits, backend } from './support/fixture'

const mac = process.platform === 'darwin'

const rail = (page: Page) =>
  page.getByRole('navigation', { name: 'Main navigation' })

/** The space's pages in the rail, by the names the rail gives them. */
const PAGES = [
  ['Home', 'home'],
  ['History', 'history'],
  ['Insights', 'insights'],
  ['Backup', 'backup'],
] as const

/** Browsers with at least one profile backed up, counted the way Home's Sources card counts them. */
async function backedUpBrowsers() {
  const snapshot = await backend<AppSnapshot>('app_snapshot')
  const stats =
    await backend<{ browserName: string; profileId: string }[]>(
      'load_source_stats',
    )
  const selected = new Set(snapshot.config.selectedProfileIds)
  const browsers = new Map<string, boolean>()
  for (const profile of snapshot.browserProfiles) {
    if (!profile.historyExists) continue
    browsers.set(
      profile.browserName,
      (browsers.get(profile.browserName) ?? false) ||
        selected.has(profile.profileId),
    )
  }
  for (const stat of stats) {
    browsers.set(
      stat.browserName,
      (browsers.get(stat.browserName) ?? false) || selected.has(stat.profileId),
    )
  }
  return [...browsers.values()].filter(Boolean).length
}

/** A word from a page the fixture visited, for searches that find something. */
function fixtureWord() {
  const word = archivedVisits()
    .flatMap((visit) => (visit.title ?? '').split(/[^A-Za-z]+/))
    .find((part) => part.length >= 5)
  if (!word) throw new Error('the fixture has no page title with a long word')
  return word.toLowerCase()
}

test('#/ opens the space; the switcher names it and its menu counts it', async ({
  page,
}) => {
  await page.goto('/#/')
  await unlockIfLocked(page)
  await expect(page).toHaveURL(/#\/browsing\/home$/)

  const switcher = rail(page).getByRole('button', {
    name: 'Switch data type. Now: Browsing history',
  })
  await expect(switcher).toHaveText('Browsing')
  await switcher.click()
  const menu = page.getByRole('menu', {
    name: 'Switch data type. Now: Browsing history',
  })
  await expect(menu).toBeVisible()

  const dashboard = await backend<{ totalVisits: number }>(
    'load_dashboard_snapshot',
  )
  const browsers = await backedUpBrowsers()
  const visits = dashboard.totalVisits.toLocaleString('en-US')
  const line = `${visits} visit${dashboard.totalVisits === 1 ? '' : 's'} · ${browsers} browser${browsers === 1 ? '' : 's'}`

  // One space, the current one, with its shortcut. Agent conversations are
  // off, so they are not a space here; the menu's one other item offers to
  // keep another kind of data, and names how many more there are.
  const spaces = menu.getByRole('menuitemradio')
  await expect(spaces).toHaveCount(1)
  await expect(spaces.first()).toHaveAttribute('aria-checked', 'true')
  await expect(spaces.first()).toContainText('Browsing history')
  await expect(spaces.first()).toContainText(line)
  await expect(spaces.first()).toContainText(mac ? '⌘1' : 'Ctrl+1')
  const keepAnother = menu.getByRole('menuitem')
  await expect(keepAnother).toHaveCount(1)
  await expect(keepAnother).toHaveText(/^Keep another kind of data…\s*1 more$/)
  await page.keyboard.press('Escape')
  await expect(menu).toBeHidden()

  // It opens Settings › Manage data types.
  await switcher.click()
  await menu.getByRole('menuitem').click()
  await expect(menu).toBeHidden()
  await expect(page).toHaveURL(/#\/settings\/kinds$/)
  await expect(
    page.getByRole('heading', { name: 'Manage data types', level: 2 }),
  ).toBeVisible()
})

test('each page in the rail opens its address, and Settings and Ask keep the space', async ({
  page,
}) => {
  await page.goto('/#/browsing/home')
  await unlockIfLocked(page)
  for (const [name, path] of PAGES) {
    await rail(page).getByRole('link', { name, exact: true }).click()
    await expect(page).toHaveURL(
      new RegExp(`#/browsing/${path}(\\?.*)?$`.replaceAll('/', '\\/')),
    )
    await expect(
      rail(page).getByRole('link', { name, exact: true }),
    ).toHaveAttribute('aria-current', 'page')
  }

  for (const [shared, path] of [
    ['Settings', /#\/settings$/],
    ['Ask', /#\/ask(\?.*)?$/],
  ] as const) {
    await rail(page).getByRole('link', { name: shared, exact: true }).click()
    await expect(page).toHaveURL(path)
    await expect(
      rail(page).getByRole('button', {
        name: 'Switch data type. Now: Browsing history',
      }),
    ).toBeVisible()
    for (const [name, pagePath] of PAGES) {
      await expect(
        rail(page).getByRole('link', { name, exact: true }),
      ).toHaveAttribute('href', `#/browsing/${pagePath}`)
    }
  }

  // ⌘1 / Ctrl+1: the first space, from a screen that has none.
  await rail(page).getByRole('link', { name: 'Settings', exact: true }).click()
  await expect(page).toHaveURL(/#\/settings$/)
  await page.keyboard.press(mac ? 'Meta+1' : 'Control+1')
  await expect(page).toHaveURL(/#\/browsing\/home$/)
})

test('⌘K: the space’s pages under Go to, and a search of the space on its list page', async ({
  page,
}) => {
  await page.goto('/#/browsing/insights')
  await unlockIfLocked(page)
  await page.keyboard.press('ControlOrMeta+k')
  const palette = page.getByRole('dialog')
  const goTo = palette.getByRole('group', { name: 'Go to' })
  for (const [name] of PAGES)
    await expect(
      goTo.getByRole('option', { name: `Browsing · ${name}` }),
    ).toBeVisible()
  // Exact: "Background tasks" also contains "ask".
  await expect(
    goTo.getByRole('option', { name: 'Ask', exact: true }),
  ).toBeVisible()
  await expect(
    goTo.getByRole('option', { name: 'Settings', exact: true }),
  ).toBeVisible()
  // One space: nothing to switch to.
  await expect(palette.getByRole('group', { name: 'Switch to' })).toHaveCount(0)

  const word = fixtureWord()
  await palette.getByRole('combobox').fill(word)
  const results = palette.getByRole('group', { name: 'Pages' })
  // Pages whose words match, then the search of the whole space.
  await expect(results.getByRole('option').first()).toBeVisible()
  const search = results.getByRole('option', {
    name: `Search “Browsing history” for “${word}”`,
  })
  await expect(search).toBeVisible()
  await search.click()
  await expect(page).toHaveURL(
    new RegExp(`#/browsing/history\\?q=${word}$`.replaceAll('/', '\\/')),
  )
  await expect(
    page.getByRole('searchbox', { name: 'Search history' }),
  ).toHaveValue(word)
})

test('the language is chosen in Settings, and the rail and ⌘K follow it', async ({
  page,
}) => {
  await page.goto('/#/settings/general')
  await unlockIfLocked(page)
  const before = await backend<AppSnapshot>('app_snapshot')
  const stored = await page.evaluate(() =>
    localStorage.getItem('pathkeep-language-preference'),
  )
  // Nothing in the rail chooses a language any more.
  await expect(
    rail(page).getByRole('button', { name: /language/i }),
  ).toHaveCount(0)
  await expect(rail(page).getByText(/^(A|EN|简|繁)$/)).toHaveCount(0)

  try {
    await page.getByRole('combobox', { name: 'Language' }).click()
    await page.getByRole('option', { name: '繁體中文' }).click()
    const zhRail = page.getByRole('navigation', { name: '主導覽' })
    await expect(zhRail.getByRole('link', { name: '紀錄' })).toBeVisible()
    await expect(zhRail.getByRole('link', { name: 'History' })).toHaveCount(0)
    await expect(
      zhRail.getByRole('button', { name: '切換資料類型。現在：瀏覽紀錄' }),
    ).toHaveText('瀏覽紀錄')

    await page.keyboard.press('ControlOrMeta+k')
    const palette = page.getByRole('dialog')
    await expect(
      palette
        .getByRole('group', { name: '前往' })
        .getByRole('option', { name: '瀏覽紀錄 · 紀錄' }),
    ).toBeVisible()
    const word = fixtureWord()
    await palette.getByRole('combobox').fill(word)
    const search = palette.getByRole('option', {
      name: `在「瀏覽紀錄」搜尋「${word}」`,
    })
    await expect(search).toBeVisible()
    await search.click()
    await expect(page).toHaveURL(
      new RegExp(`#/browsing/history\\?q=${word}$`.replaceAll('/', '\\/')),
    )

    // And back: English names the page History again.
    await page.goto('/#/settings/general')
    await page.getByRole('combobox', { name: '語言' }).click()
    await page.getByRole('option', { name: 'English' }).click()
    await expect(
      rail(page).getByRole('link', { name: 'History', exact: true }),
    ).toBeVisible()
    await expect(rail(page).getByRole('link', { name: '紀錄' })).toHaveCount(0)
  } finally {
    // Every spec after this one reads the English interface.
    const now = await backend<AppSnapshot>('app_snapshot')
    await backend('save_config', {
      config: {
        ...now.config,
        preferredLanguage: before.config.preferredLanguage,
      },
    })
    await page.evaluate((value) => {
      if (value === null)
        localStorage.removeItem('pathkeep-language-preference')
      else localStorage.setItem('pathkeep-language-preference', value)
    }, stored)
  }
})
