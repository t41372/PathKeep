/**
 * Ask, against an OpenAI-compatible service (`support/fake-model.ts`).
 *
 * Proves: a service added in Settings → AI passes its connection test, and
 * the request carries its key. Asked a question, Ask gives the model
 * PathKeep's tools, runs the search the model asks for on the archive, sends
 * the results back, and shows the answer with the pages it found: the
 * fixture's page about spawn_blocking is cited, and its address is in what the
 * model was sent. An Insights card's "Ask about this" opens Ask with a new
 * chat whose box holds the card's question and dates, sent only when the
 * user sends it. A wrong key and an address nothing listens on each fail the
 * connection test with the reason in words, not the backend's error text; the
 * service with the wrong key, edited in place with the right one, connects.
 */
import { expect, test } from '@playwright/test'
import { addService } from './support/ai-service'
import {
  FAKE_ANSWER,
  chatRequests,
  startFakeModel,
  textOf,
} from './support/fake-model'

const KEY = 'e2e-test-key'

/** The fixture's GitHub issue about spawn_blocking (the rust topic). */
const ISSUE_TITLE = 'spawn_blocking pool exhaustion · tokio-rs/tokio'
const ISSUE_URL = 'tokio-rs/tokio/issues/5124'

test('Ask answers from a search of the archive, with the pages it found', async ({
  page,
}) => {
  const model = await startFakeModel({
    apiKey: KEY,
    searchFor: 'spawn_blocking',
  })
  try {
    await addService(page, { name: 'E2E model', address: model.url, key: KEY })
    await page.getByRole('button', { name: 'Test connection' }).click()
    await expect(
      page.getByText(/^Connected to e2e-model in \d+ ms\.$/),
    ).toBeVisible({ timeout: 30_000 })

    await page.goto('/#/ask')
    const composer = page.getByRole('textbox', { name: 'Message' })
    await composer.fill('Where did I read about spawn_blocking?')
    await composer.press('Enter')
    await expect(page.getByText(FAKE_ANSWER)).toBeVisible({ timeout: 60_000 })
    await expect(
      page
        .getByLabel('Pages used')
        .getByRole('link', { name: ISSUE_TITLE, exact: false }),
    ).toBeVisible()

    // What PathKeep sent: the key every time, its tools, then the search's results.
    const chats = chatRequests(model)
    expect(
      chats.every((request) => request.authorization === `Bearer ${KEY}`),
    ).toBe(true)
    const asked = chats.find((request) => request.body.stream)
    expect(asked?.body.tools?.map((tool) => tool.function?.name)).toContain(
      'search_history',
    )
    const answered = chats.find((request) =>
      request.body.messages?.some((message) => message.role === 'tool'),
    )
    const results = answered?.body.messages
      ?.filter((message) => message.role === 'tool')
      .map((message) => textOf(message.content))
      .join('\n')
    expect(results).toContain(ISSUE_URL)

    // From an Insights card: its question waits in a new chat.
    const sentBefore = model.requests.length
    await page.goto('/#/insights?range=d30')
    await page
      .getByRole('heading', { name: 'Top sites', exact: true })
      .locator('xpath=ancestor::section[1]')
      .getByRole('button', { name: 'About this number' })
      .click()
    await page
      .getByRole('dialog')
      .getByRole('link', { name: 'Ask about this' })
      .click()
    await expect(page).toHaveURL(/#\/ask$/)
    await expect(page.getByRole('textbox', { name: 'Message' })).toHaveValue(
      /^What stands out in “Top sites” in my browsing from .+ to .+\?$/,
    )
    expect(model.requests.length, 'nothing is sent before Send').toBe(
      sentBefore,
    )
  } finally {
    await model.close()
  }
})

test('the connection test says why it could not connect, and an edited key fixes it', async ({
  page,
}) => {
  const model = await startFakeModel({ apiKey: KEY, searchFor: 'anything' })
  try {
    await addService(page, {
      name: 'E2E wrong key',
      address: model.url,
      key: 'not-the-key',
    })
    await page.getByRole('button', { name: 'Test connection' }).click()
    await expect(
      page.getByText(
        'Could not connect. The service did not accept the API key. Check the key and test again.',
      ),
    ).toBeVisible({ timeout: 30_000 })

    // The same service, its key changed in place: the rest stays as saved.
    await page.getByRole('button', { name: 'Edit', exact: true }).click()
    const edit = page.getByRole('dialog', { name: 'Edit E2E wrong key' })
    await expect(edit.getByRole('textbox', { name: 'Address' })).toHaveValue(
      model.url,
    )
    await expect(edit.getByRole('textbox', { name: 'Model' })).toHaveValue(
      'e2e-model',
    )
    await edit.getByLabel('API key').fill(KEY)
    await edit.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('E2E wrong key is saved.')).toBeVisible()
    // Saving runs nothing; the test does.
    await page.getByRole('button', { name: 'Test connection' }).click()
    await expect(
      page.getByText(/^Connected to e2e-model in \d+ ms\.$/),
    ).toBeVisible({ timeout: 30_000 })

    // Nothing listens on port 9 (discard) on this machine.
    const nowhere = 'http://127.0.0.1:9/v1'
    await addService(page, { name: 'E2E nowhere', address: nowhere, key: KEY })
    await page.getByRole('button', { name: 'Test connection' }).click()
    await expect(
      page.getByText(
        `Could not connect. Could not reach ${nowhere}. Check the address and that the service is running.`,
      ),
    ).toBeVisible({ timeout: 30_000 })
  } finally {
    await model.close()
  }
})
