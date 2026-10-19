/**
 * Responsibilities: a labelled related-page corpus and real browser-file import actions.
 * Not responsible for: ranking, replacing archive reads or mutating the archive directly.
 * Failure modes: indistinguishable titles make the test pass with unrelated or duplicate rows.
 * Tests: semantic-index.spec.ts reads this corpus through History's real detail panel.
 */
import { expect, type Page } from '@playwright/test'
import { unlockIfLocked } from './app'
import type { FixtureVisit } from './fixture'

export const RELATED_SOURCE = {
  url: 'https://alpha.example.org/related/source',
  title: 'Related subject A source',
}
export const RELATED_LATE = {
  url: 'https://gamma.example.com/related/late',
  title: 'Related subject A new arrival',
}
export const RELATED_HIDDEN = {
  url: 'https://gamma.example.com/related/hidden',
  title: 'Related subject A hidden article',
}

/** The same subject has four distinct own-site pages and three on other sites, after folding. */
export function relatedVisits(now: number): FixtureVisit[] {
  const pages = [
    ...['one', 'two', 'three', 'four'].map((name) => ({
      url: `https://alpha.example.org/related/${name}`,
      title: `Related subject A own ${name}`,
    })),
    {
      url: 'https://beta.example.net/related/toolkit',
      title: 'Related subject A toolkit',
    },
    {
      url: 'https://beta.example.net/related/toolkit?page=2',
      title: 'Related subject A toolkit | Board',
    },
    {
      url: 'https://beta.example.net/related/library',
      title: 'Related subject A library',
    },
    {
      url: 'https://gamma.example.com/related/docs',
      title: 'Related subject A documentation',
    },
    ...['alpha.example.org', 'beta.example.net', 'gamma.example.com'].map(
      (site) => ({
        url: `https://${site}/related/other-subject`,
        title: `Related subject B ${site}`,
      }),
    ),
    RELATED_SOURCE,
  ]
  return pages.map((page, i) => ({
    ...page,
    at: now - (pages.length - i) * 5000,
    typed: true,
    duration: 1000,
  }))
}

/** Keeps the service's transport scripted while all vector and archive computation stays real. */
export function relatedVector(text: string, dim: number): number[] {
  const vector = new Array<number>(dim).fill(0)
  vector[
    text.startsWith('Related subject A ')
      ? 0
      : text.startsWith('Related subject B ')
        ? 1
        : 2
  ] = 1
  return vector
}

/** Goes through Backup's file preview and explicit import button. */
export async function importRelatedFile(
  page: Page,
  file: string,
  count: number,
) {
  await page.goto('/#/backup')
  await unlockIfLocked(page)
  const card = page
    .getByRole('heading', { name: 'Import a browser’s history' })
    .locator('xpath=ancestor::section[1]')
  const another = card.getByRole('button', { name: 'Import another' })
  if (await another.isVisible()) await another.click()
  const field = page.getByRole('textbox', {
    name: 'Path to a history file or profile folder',
  })
  await field.fill(file)
  await field.press('Enter')
  await expect(page.getByTestId('browser-import-preview')).toBeVisible({
    timeout: 30_000,
  })
  await page
    .getByRole('button', {
      name: `Import ${count} ${count === 1 ? 'visit' : 'visits'}`,
      exact: true,
    })
    .click()
  await expect(
    page.getByText(`Imported ${count} new visit`, { exact: false }).first(),
  ).toBeVisible({ timeout: 60_000 })
}

/** Undo leaves the indexed evidence in place, so related-page hydration must recheck visibility. */
export async function undoRelatedFile(page: Page, filename: string) {
  await page.goto('/#/backup')
  await unlockIfLocked(page)
  const batch = page
    .getByRole('heading', { name: 'Recent imports' })
    .locator('xpath=ancestor::section[1]')
    .getByRole('listitem')
    .filter({ hasText: filename })
  await batch.getByRole('button', { name: 'Undo', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: /^Import from / })
  await expect(dialog).toContainText(filename)
  await dialog.getByRole('button', { name: 'Undo import', exact: true }).click()
  await expect(dialog).toBeHidden({ timeout: 60_000 })
}
