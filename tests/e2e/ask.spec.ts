/**
 * Ask, against an OpenAI-compatible service (`support/fake-model.ts`).
 *
 * Proves: a service added in Settings → AI passes its connection test, and
 * the request carries its key. Asked a question, Ask gives the model
 * PathKeep's tools, runs the search the model asks for on the archive, sends
 * the results back, and shows the answer with the pages it found: the
 * fixture's page about spawn_blocking is cited, and its address is in what the
 * model was sent. A wrong key and an address nothing listens on each fail the
 * connection test with the reason in words, not the backend's error text.
 */
import { expect, test, type Page } from '@playwright/test'
import { FAKE_ANSWER, startFakeModel } from './support/fake-model'

const KEY = 'e2e-test-key'

/** The fixture's GitHub issue about spawn_blocking (the rust topic). */
const ISSUE_TITLE = 'spawn_blocking pool exhaustion · tokio-rs/tokio'
const ISSUE_URL = 'tokio-rs/tokio/issues/5124'

async function addService(
  page: Page,
  { name, address, key }: { name: string; address: string; key: string },
) {
  await page.goto('/#/settings/ai')
  await page.getByRole('combobox', { name: 'AI service' }).click()
  await page.getByRole('option', { name: 'Add provider…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Add an AI service' })
  await dialog.getByRole('combobox', { name: 'Service' }).click()
  await page.getByRole('option', { name: 'OpenAI or compatible' }).click()
  await dialog.getByRole('textbox', { name: 'Address' }).fill(address)
  await dialog.getByRole('textbox', { name: 'Model' }).fill('e2e-model')
  await dialog.getByRole('textbox', { name: 'Name' }).fill(name)
  await dialog.getByLabel('API key').fill(key)
  await dialog.getByRole('button', { name: 'Add service' }).click()
  await expect(page.getByText(`${name} is ready.`)).toBeVisible()
  await expect(
    page.getByRole('combobox', { name: 'AI service' }),
  ).toContainText(name)
}

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
    const chats = model.requests.filter((request) =>
      request.path.endsWith('/chat/completions'),
    )
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
      .map((message) => JSON.stringify(message.content))
      .join('\n')
    expect(results).toContain(ISSUE_URL)
  } finally {
    await model.close()
  }
})

test('the connection test says why it could not connect', async ({ page }) => {
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
