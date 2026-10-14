/**
 * Setting up an embeddings service the way people do with LM Studio.
 *
 * Proves: Settings says which model the app comes with. Adding LM Studio
 * with the address its own window shows (no `/v1`) lists the service's
 * models, only the embedding ones, with which is loaded and how much each can
 * read; the one picked passes the connection test, so the address was
 * completed. An address that reaches the server but not its API fails with
 * what to fix, although LM Studio answers such a request with HTTP 200 and an
 * error rather than a 404. Changing the model says, before saving, that the
 * index is built again and how many pages that is (here when semantic search
 * is turned on: it is off in this run, and building is `semantic-index.spec`'s).
 *
 * The service runs on this computer (`support/fake-embeddings.ts`) and
 * answers like LM Studio: its model list, and HTTP 200 with an `error` for a
 * path it does not serve.
 */
import { expect, test } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { startFakeEmbeddings } from './support/fake-embeddings'
import { backend } from './support/fixture'

interface Config {
  ai: unknown
}

const loadConfig = async () =>
  (await backend<{ config: Config }>('app_snapshot')).config

test('an LM Studio service is set up from its own address and model list', async ({
  page,
}, testInfo) => {
  const service = await startFakeEmbeddings({
    models: [
      {
        id: 'e2e-embed-small',
        type: 'embeddings',
        state: 'loaded',
        max_context_length: 2048,
        loaded_context_length: 2048,
      },
      {
        id: 'e2e-embed-large',
        type: 'embeddings',
        state: 'not-loaded',
        max_context_length: 32_768,
      },
      {
        id: 'e2e-chat',
        type: 'llm',
        state: 'loaded',
        max_context_length: 32_768,
        loaded_context_length: 8192,
      },
    ],
  })
  // What LM Studio's window shows: the server, without `/v1`.
  const shown = service.url.replace(/\/v1$/, '')
  const before = await loadConfig()
  try {
    await page.goto('/#/settings/ai')
    await unlockIfLocked(page)
    await expect(
      page.getByText(
        /^potion-multilingual-128M, 256 dimensions\. It comes with PathKeep/,
      ),
    ).toBeVisible()

    await page.getByRole('combobox', { name: 'Model' }).click()
    await page
      .getByRole('option', { name: 'Add an embeddings service…' })
      .click()
    const dialog = page.getByRole('dialog', {
      name: 'Add an embeddings service',
    })
    await dialog.getByRole('combobox', { name: 'Service' }).click()
    await page
      .getByRole('option', { name: 'LM Studio (this computer)' })
      .click()
    await dialog.getByRole('textbox', { name: 'Address' }).fill(shown)
    await expect(
      dialog.getByText('2 models on this service. Pick one, or type a name.'),
    ).toBeVisible()
    await dialog.getByRole('combobox', { name: 'Browse the list' }).click()
    const models = page.getByRole('option')
    await expect(models).toHaveCount(2)
    await testInfo.attach('the model list', {
      body: await page.screenshot(),
      contentType: 'image/png',
    })
    await expect(models.filter({ hasText: 'e2e-embed-small' })).toContainText(
      'Loaded · reads 2K',
    )
    await expect(models.filter({ hasText: 'e2e-embed-large' })).toContainText(
      'up to 32K',
    )
    await models.filter({ hasText: 'e2e-embed-small' }).click()
    await expect(dialog.getByRole('textbox', { name: 'Model' })).toHaveValue(
      'e2e-embed-small',
    )
    await dialog.getByRole('textbox', { name: 'Name' }).fill('LM Studio E2E')
    await dialog.getByRole('button', { name: 'Add service' }).click()
    await expect(page.getByText('LM Studio E2E is ready.')).toBeVisible()
    await expect(
      page.getByText(/^Connected to e2e-embed-small in \d+ ms\.$/),
    ).toBeVisible({ timeout: 30_000 })

    // An address on the server that is not its API.
    const wrong = `${shown}/lmstudio`
    await page.getByRole('button', { name: 'Edit' }).click()
    const edit = page.getByRole('dialog', { name: 'Edit LM Studio E2E' })
    await edit.getByRole('textbox', { name: 'Address' }).fill(wrong)
    await edit.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('LM Studio E2E is saved.')).toBeVisible()
    await expect(
      page.getByText(
        `Could not connect. ${wrong} reaches a server, but not its API. For LM Studio and Ollama the address usually ends in /v1, like http://localhost:1234/v1.`,
      ),
    ).toBeVisible({ timeout: 30_000 })

    // Another model: the cost is said before anything is saved.
    const { pageCount } = await backend<{ pageCount: number }>(
      'estimate_reembed',
      { scope: 'full' },
    )
    await page.getByRole('button', { name: 'Edit' }).click()
    await edit.getByRole('textbox', { name: 'Address' }).fill(shown)
    await edit.getByRole('combobox', { name: 'Browse the list' }).click()
    await page
      .getByRole('option')
      .filter({ hasText: 'e2e-embed-large' })
      .click()
    await testInfo.attach('changing the model', {
      body: await page.screenshot(),
      contentType: 'image/png',
    })
    await expect(edit.getByRole('note')).toHaveText(
      `Another model means building the semantic index again when semantic search is on: ${pageCount.toLocaleString('en-US')} pages. The index made with e2e-embed-small is deleted then.`,
    )
    await edit.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(
      page.getByText(/^Connected to e2e-embed-large in \d+ ms\.$/),
    ).toBeVisible({ timeout: 30_000 })
    await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
      'LM Studio E2E · e2e-embed-large',
    )
  } finally {
    const current = await loadConfig()
    await backend('save_config', { config: { ...current, ai: before.ai } })
    await service.close()
  }
})
