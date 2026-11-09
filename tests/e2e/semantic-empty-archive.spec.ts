/**
 * Semantic search turned on before the archive holds a visit: a new user
 * whose browser has no history yet sets up PathKeep, then turns it on.
 *
 * Proves:
 * - The build over the empty archive ends and sends the service nothing, and
 *   Settings says nothing is indexed yet. It does not call the index
 *   incomplete and ask for a rebuild: a rebuild would make the same empty
 *   index again.
 * - Once the browser has visits, the next backup brings the index up to date
 *   by itself, and History's semantic search finds the page by its words.
 *
 * Runs after wipe.spec, on the empty data folder it leaves, and deletes all
 * data at the end, as a user does in Settings, so upgrade.spec finds the
 * folder empty again. The Chrome profile with no history is this test's own:
 * added to the fixture's Chrome folder at the start, removed at the end.
 */
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Locator } from '@playwright/test'
import {
  appendChromeVisits,
  writeChromeHistory,
} from '../../scripts/fixtures/synthetic-browsers.mjs'
import { addEmbeddingsService } from './support/ai-service'
import { startFakeEmbeddings } from './support/fake-embeddings'
import { backend, fixture } from './support/fixture'

/** A page with words no fixture page has, so a search for them finds only it. */
const FIRST_PAGE = {
  url: 'https://lighthouse.example.org/logbook/fog-signal-schedule',
  title: 'Lighthouse fog signal schedule',
}

test('semantic search turned on before the first visit says nothing is indexed yet, then catches up', async ({
  page,
}, testInfo) => {
  test.setTimeout(300_000)
  const chromeRoot = fixture().chromeUserDataRoot
  const localState = path.join(chromeRoot, 'Local State')
  const originalLocalState = readFileSync(localState, 'utf8')
  const profileDir = path.join(chromeRoot, 'Profile 2')
  const history = path.join(profileDir, 'History')
  const service = await startFakeEmbeddings()
  try {
    // A Chrome profile with no history yet, as on a new computer.
    mkdirSync(profileDir, { recursive: true })
    writeChromeHistory(history, [])
    const state = JSON.parse(originalLocalState) as {
      profile: { info_cache: Record<string, unknown> }
    }
    state.profile.info_cache['Profile 2'] = {
      name: 'Fresh',
      user_name: 'fresh@example.test',
    }
    writeFileSync(localState, JSON.stringify(state))

    // Onboarding backs up only that profile: there is nothing to save.
    await page.goto('/')
    await expect(
      page.getByRole('heading', {
        name: 'Your browser is deleting your history',
      }),
    ).toBeVisible()
    await page.getByRole('button', { name: 'Get started' }).click()
    await expect(
      page.getByRole('heading', { name: 'Which browsers should we back up?' }),
    ).toBeVisible()
    for (const checkbox of await page.getByRole('checkbox').all()) {
      const name =
        (await checkbox.getAttribute('aria-label')) ?? (await labelOf(checkbox))
      const fresh = /Google Chrome\s*Fresh/.test(name)
      if ((await checkbox.isChecked()) !== fresh) await checkbox.click()
    }
    await page.getByRole('button', { name: 'Continue' }).click()
    await expect(
      page.getByRole('heading', { name: 'Where the archive lives' }),
    ).toBeVisible()
    await page.getByRole('button', { name: 'Continue' }).click()
    await expect(
      page.getByRole('heading', { name: 'Encrypt the archive?' }),
    ).toBeVisible()
    await page.getByRole('radio', { name: /^Don’t encrypt/ }).click()
    await page.getByRole('button', { name: 'Continue' }).click()
    // No schedule: installing one would talk to the OS scheduler.
    await expect(
      page.getByRole('heading', { name: 'How often should we back up?' }),
    ).toBeVisible()
    await page.getByRole('radio', { name: /^Manual/ }).click()
    await page.getByRole('button', { name: 'Continue' }).click()
    await expect(
      page.getByRole('heading', { name: 'Turn on AI features?' }),
    ).toBeVisible()
    await page.getByRole('radio', { name: /^Not now/ }).click()
    await page.getByRole('button', { name: 'Continue' }).click()
    await page.getByRole('button', { name: 'Start first backup' }).click()
    await expect(page.getByText('Done. 0 visits saved.')).toBeVisible({
      timeout: 90_000,
    })
    await page.getByRole('button', { name: 'Open PathKeep' }).click()

    // Semantic search on, with a service: the build has nothing to read.
    await addEmbeddingsService(page, service.url, 'e2e-embed', 'E2E embeddings')
    const semantic = page.getByRole('switch', { name: 'Semantic search' })
    await semantic.click()
    await expect(semantic).toBeChecked()
    await expect
      .poll(
        async () =>
          (
            await backend<{ semanticIndex: { chosenComplete: boolean } }>(
              'load_ai_queue_status',
            )
          ).semanticIndex.chosenComplete,
        { timeout: 120_000 },
      )
      .toBe(true)
    await expect(page.getByText('Nothing indexed yet.')).toBeVisible({
      timeout: 30_000,
    })
    await expect(
      page.getByText('The index is incomplete. Rebuild it to fix this.'),
    ).toHaveCount(0)
    await testInfo.attach('index status after the build over nothing', {
      body: JSON.stringify(
        (await backend<{ aiStatus: unknown }>('app_snapshot')).aiStatus,
        null,
        2,
      ),
      contentType: 'application/json',
    })
    expect(service.inputs, 'nothing to embed').toEqual([])

    // The browser's first visit: the next backup indexes it by itself.
    appendChromeVisits(history, [
      { ...FIRST_PAGE, at: Date.now() - 60_000, typed: true, duration: 30_000 },
    ])
    await page.goto('/#/backup')
    await page.getByRole('button', { name: 'Back up now' }).first().click()
    await expect(
      page
        .getByRole('heading', { name: 'Recent runs' })
        .locator('xpath=ancestor::*[.//li][1]')
        .getByRole('listitem')
        .first(),
    ).toContainText(/\b1 new visit/, { timeout: 60_000 })
    await expect
      .poll(
        () => service.inputs.some((text) => text.startsWith(FIRST_PAGE.title)),
        { timeout: 120_000 },
      )
      .toBe(true)
    await page.goto('/#/settings/ai')
    await expect(
      page.getByText('1 visit indexed', { exact: false }),
    ).toBeVisible({ timeout: 60_000 })

    await page.goto('/#/history')
    await page
      .getByRole('searchbox', { name: 'Search history' })
      .fill('fog signal lighthouse')
    await page.getByRole('radio', { name: 'Semantic' }).click()
    await expect(
      page
        .getByRole('listbox', { name: 'History results' })
        .getByRole('option')
        .first(),
    ).toContainText(FIRST_PAGE.title, { timeout: 60_000 })
    // The only page matches by its words too: what shows that meaning answered
    // is the search going to the service and the line naming the index.
    await expect
      .poll(() => service.inputs.includes('fog signal lighthouse'), {
        timeout: 30_000,
      })
      .toBe(true)
    await expect(
      page.getByText(
        'Semantic search with E2E embeddings · e2e-embed · 1 visit indexed · updated',
      ),
    ).toBeVisible()

    // Leave the data folder as wipe.spec left it, the way a user deletes it.
    await page.goto('/#/settings/storage')
    await page.getByRole('button', { name: 'Delete…' }).click()
    const dialog = page.getByRole('dialog', { name: 'Delete all data?' })
    await dialog.getByRole('textbox').fill('DELETE')
    await dialog.getByRole('button', { name: 'Delete forever' }).click()
    await expect(
      page.getByRole('heading', {
        name: 'Your browser is deleting your history',
      }),
    ).toBeVisible({ timeout: 60_000 })
  } finally {
    await service.close()
    writeFileSync(localState, originalLocalState)
    rmSync(profileDir, { recursive: true, force: true })
  }
})

async function labelOf(checkbox: Locator) {
  const id = await checkbox.getAttribute('id')
  const label = checkbox.page().locator(`label[for="${id}"]`)
  return (await label.innerText()).replace(/\s+/g, ' ')
}
