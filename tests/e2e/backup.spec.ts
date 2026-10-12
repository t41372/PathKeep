/**
 * Backup: new browsing shows up after the next backup, and a paused source
 * is left alone until it is turned back on. A run that had to leave
 * something out says why.
 *
 * Proves: a backup appends exactly the visits added since the last one (no
 * re-import, no loss), a paused profile contributes nothing even when it
 * has new visits, and turning it back on catches up exactly those visits.
 * The run sheet lists every warning the run recorded, including those from
 * the work after the backup itself, as sentences in the chosen language with
 * the fix beside them; notes on side work sit under the summary instead.
 */
import { existsSync, renameSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import {
  appendChromeVisits,
  appendFirefoxVisits,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import { backend, fixture } from './support/fixture'

const NEW_URL = 'https://e2e.example/backup-check'
const NEW_TITLE = 'Backup check page'

function visitsNow(count: number, offsetMs = 0) {
  const now = Date.now() + offsetMs
  return Array.from({ length: count }, (_, index) => ({
    url: NEW_URL,
    title: NEW_TITLE,
    at: now - (count - index) * 60_000,
    typed: false,
    duration: 30_000,
  }))
}

function latestRun(page: Page, heading = 'Recent runs') {
  return page
    .getByRole('heading', { name: heading })
    .locator('xpath=ancestor::*[.//li][1]')
    .getByRole('listitem')
    .first()
}

async function backUp(page: Page, expected: RegExp) {
  await page.goto('/#/backup')
  await page.getByRole('button', { name: 'Back up now' }).first().click()
  await expect(latestRun(page)).toContainText(expected, { timeout: 60_000 })
}

interface Config {
  preferredLanguage: string
  ai: {
    enabled: boolean
    semanticIndexEnabled: boolean
    autoIndexAfterBackup: boolean
    jobQueuePaused: boolean
  }
}

async function loadConfig() {
  return (await backend<{ config: Config }>('app_snapshot')).config
}

/**
 * Searches History for the test page and waits for exactly `expected` visits
 * to it. Search lists the page once; the header counts its visits
 * ("1 page · 5 visits · Full text").
 */
async function expectSearchHits(page: Page, expected: number) {
  await page.goto('/#/history')
  await page.getByRole('searchbox', { name: 'Search history' }).fill(NEW_TITLE)
  // With no match both the header and the empty state show; the header comes first.
  const header = page.getByText(/visits? · Full text$|^Nothing found$/).first()
  await expect
    .poll(async () => {
      const text = await header.innerText().catch(() => '')
      if (text === 'Nothing found') return 0
      const counts = text.match(
        /^([01]) pages? · ([\d,]+) visits? · Full text$/,
      )
      return counts ? Number(counts[2].replace(/[^\d]/g, '')) : Number.NaN
    })
    .toBe(expected)
}

test('new visits arrive with the next backup; a paused source waits', async ({
  page,
}) => {
  const { files } = fixture()
  await expectSearchHits(page, 0)

  appendChromeVisits(files['chrome:Default'], visitsNow(3))
  appendFirefoxVisits(files['firefox:k3x9.default-release'], visitsNow(2))
  await backUp(page, /\b5 new visits/)
  await expectSearchHits(page, 5)

  // Pause Firefox, browse some more in it, back up: nothing new.
  await page.goto('/#/backup')
  const firefox = page.getByRole('switch', {
    name: 'Back up Firefox default-release',
  })
  await firefox.click()
  await expect(firefox).not.toBeChecked()
  appendFirefoxVisits(
    files['firefox:k3x9.default-release'],
    visitsNow(4, 1_000),
  )
  await backUp(page, /\b0 new visits/)
  await expectSearchHits(page, 5)

  // Turn it back on: exactly the four waiting visits come in.
  await page.goto('/#/backup')
  await firefox.click()
  await expect(firefox).toBeChecked()
  await backUp(page, /\b4 new visits/)
  await expectSearchHits(page, 9)
})

test('a run that left things out says why, in your language, beside the fix', async ({
  page,
}) => {
  const { files } = fixture()
  const work = files['chrome:Profile 1']
  const away = `${work}.away`
  const before = await loadConfig()
  try {
    // Chrome's Work profile lost its history file; Personal gained a search.
    renameSync(work, away)
    appendChromeVisits(files['chrome:Default'], [
      {
        url: 'https://www.google.com/search?q=run+sheet+warnings',
        title: 'run sheet warnings - Google Search',
        term: 'run sheet warnings',
        at: Date.now() - 60_000,
        typed: false,
        duration: 5_000,
      },
    ])
    // Semantic search indexes after each backup, and background work is
    // paused. The UI only turns semantic search on through a model download.
    const config = await loadConfig()
    config.ai = {
      ...config.ai,
      enabled: true,
      semanticIndexEnabled: true,
      autoIndexAfterBackup: true,
      jobQueuePaused: true,
    }
    await backend('save_config', { config })

    // Backed up from Home: the toast says something needs a look and opens the run.
    await page.goto('/#/')
    await page.reload()
    await page.getByRole('button', { name: 'Back up now' }).first().click()
    const toast = page
      .locator('[data-sonner-toast]')
      .filter({ hasText: 'Backup done · 1 new visit' })
    await expect(toast).toContainText(/\d+ warnings?: /, { timeout: 60_000 })
    await toast.getByRole('button', { name: 'Details' }).click()
    await expect(page).toHaveURL(/#\/backup\?run=\d+$/)
    const sheet = page.getByRole('dialog')
    await expect(
      sheet.getByText('Saved 1 search typed in Google Chrome Personal.'),
    ).toBeVisible()
    // Recorded after the backup itself finished, so it proves those reach the run.
    const queued = sheet.getByRole('listitem').filter({
      hasText:
        'New pages are queued for semantic search. They’ll be indexed when background work resumes.',
    })
    await expect(queued).toBeVisible()

    const tab = sheet.getByRole('tab', { name: /^Warnings \(\d+\)$/ })
    await tab.click()
    const warnings = sheet.getByRole('tabpanel')
    await expect(
      warnings.getByText(
        'Skipped Google Chrome Work: its history file is missing or can’t be read.',
      ),
    ).toBeVisible()
    // The count on the tab is the whole list, and a note is not a warning.
    const listed = await warnings.getByRole('listitem').count()
    await expect(tab).toHaveText(`Warnings (${listed})`)
    await expect(
      warnings.getByText(/^Saved 1 search|^New pages are/),
    ).toHaveCount(0)

    await sheet.getByRole('tab', { name: 'Summary' }).click()
    await queued.getByRole('link', { name: 'Go to Background work' }).click()
    await expect(page).toHaveURL(/#\/settings\/background$/)

    await page.goto('/#/settings/general')
    await page.getByRole('combobox', { name: 'Language' }).click()
    await page.getByRole('option', { name: '繁體中文' }).click()
    await page.goto('/#/backup')
    await latestRun(page, '最近記錄').getByRole('button').click()
    await sheet.getByRole('tab', { name: `警告（${listed}）` }).click()
    await expect(
      sheet
        .getByRole('tabpanel')
        .getByText('已略過 Google Chrome Work：它的歷史檔案不存在或無法讀取。'),
    ).toBeVisible()
  } finally {
    if (existsSync(away)) renameSync(away, work)
    const config = await loadConfig()
    await backend('save_config', {
      config: {
        ...config,
        preferredLanguage: before.preferredLanguage,
        ai: before.ai,
      },
    })
    // Background work resumes the way the Settings switch does it: the queues
    // reload, which starts their workers on what the paused backup queued.
    await backend('load_intelligence_runtime')
  }
})
