/**
 * What Ask sends the model and how the conversation can be steered, against
 * scripted services on this computer (`support/fake-model.ts`). Whether a
 * real model answers better with it is tried by hand against LM Studio; this
 * proves what reaches the model and what the person sees.
 *
 * Proves:
 * - The model gets one system message, first, with the rules (reply in the
 *   language of the latest message, choose when the choice is left to it)
 *   and the user's own instructions from Settings after them; each request
 *   leaves room for the answer. What the model writes before it calls a
 *   tool is shown as thinking, and only its last turn as the answer, under
 *   the model that wrote it.
 * - With LM Studio, the composer shows how much of the loaded model's
 *   context the next message takes. A model loaded with too small a context
 *   turns it red with a plain warning, and the message is not sent: the
 *   answer says why. Picking another model beside the composer sends the
 *   next message to that one.
 * - Editing an earlier question answers it again and drops what came after,
 *   also once saved; a finished answer can be asked for again.
 */
import { expect, test, type Page } from '@playwright/test'
import { addLmStudioService, addService } from './support/ai-service'
import {
  chatRequests,
  lastUserText,
  offersTools,
  startFakeModel,
  textOf,
  toolResults,
  type FakeRequest,
} from './support/fake-model'
import { backend } from './support/fixture'

const KEY = 'e2e-agent-key'

const chatColumn = (page: Page) => page.getByRole('main').locator('aside + div')

async function ask(page: Page, question: string) {
  const composer = page.getByRole('textbox', { name: 'Message' })
  await composer.fill(question)
  await composer.press('Enter')
}

/** Waits until no answer is streaming. */
async function settled(page: Page) {
  await expect(
    page.getByRole('button', { name: 'Send', exact: true }),
  ).toBeVisible({ timeout: 60_000 })
}

const systemMessages = (body: FakeRequest['body']) =>
  (body.messages ?? []).filter((message) => message.role === 'system')

test('the model gets one system message with the rules, and only its last turn is the answer', async ({
  page,
}, testInfo) => {
  const before = 'Let me search the archive for tokio first.'
  const answer = '你讀的是 spawn_blocking 用完執行緒的那篇。'
  const model = await startFakeModel({
    apiKey: KEY,
    script: (body) => {
      if (body.messages?.at(-1)?.role === 'tool') return { content: answer }
      if (!offersTools(body)) return { content: 'OK' }
      return {
        content: before,
        toolCalls: [{ name: 'search_history', arguments: { query: 'tokio' } }],
      }
    },
  })
  try {
    await addService(page, {
      name: 'E2E rules',
      address: model.url,
      key: KEY,
    })
    await page.goto('/#/ask')
    await ask(page, '我之前讀過哪篇講 tokio 的文章？')
    await expect(chatColumn(page).getByText(answer)).toBeVisible({
      timeout: 60_000,
    })
    await settled(page)

    const [first, second] = chatRequests(model).filter(
      ({ body }) => (body.messages ?? []).length > 0 && offersTools(body),
    )
    const { config } = await backend<{
      config: { ai: { assistantSystemPrompt: string } }
    }>('app_snapshot')
    const systems = systemMessages(first.body)
    await testInfo.attach('first request', {
      body: JSON.stringify(first.body, null, 2),
      contentType: 'application/json',
    })
    expect(systems, 'one system message').toHaveLength(1)
    expect(first.body.messages?.[0].role, 'and it comes first').toBe('system')
    const system = textOf(systems[0].content)
    for (const rule of [
      "Reply in the language of the user's latest message.",
      'If the user leaves the choice to you, pick one reasonable thing and do it',
      'Every title, address, time or count you give must come',
    ])
      expect(system).toContain(rule)
    expect(
      system.indexOf(config.ai.assistantSystemPrompt.trim()),
      "the user's own instructions, after the rules",
    ).toBeGreaterThan(system.indexOf('Rules:'))
    // No context known for this service: room for a 4K answer.
    expect(
      (first.body as { max_tokens?: number }).max_tokens ??
        (first.body as { max_completion_tokens?: number })
          .max_completion_tokens,
    ).toBe(4096)
    expect(toolResults(second.body)).toHaveLength(1)

    // The text before the search is a step, not part of the answer: hidden
    // with the thinking until that is opened.
    await expect(chatColumn(page).getByText(before)).toBeHidden()
    await chatColumn(page)
      .getByRole('button', { name: /^Thinking/ })
      .click()
    await expect(chatColumn(page).getByText(before)).toBeVisible()
    await expect(
      chatColumn(page).getByTitle('The model that wrote this answer'),
    ).toHaveText('e2e-model')
  } finally {
    await model.close()
  }
})

test('the composer shows how full the model’s context is, and a message that does not fit is not sent', async ({
  page,
}) => {
  const model = await startFakeModel({
    apiKey: null,
    models: [
      { id: 'e2e-large', loaded_context_length: 32_768 },
      { id: 'e2e-small', loaded_context_length: 2_048 },
    ],
    script: (body) => ({ content: `Answered by ${body.model}.` }),
  })
  try {
    await addLmStudioService(page, {
      name: 'E2E LM Studio',
      address: model.url.replace(/\/v1$/, ''),
      model: 'e2e-large',
    })
    await page.goto('/#/ask')
    const meter = page
      .getByRole('textbox', { name: 'Message' })
      .locator('xpath=..')
      .getByText(/^[\d.]+K? \/ /)
    await expect(meter).toHaveText(/ \/ 32K$/, { timeout: 30_000 })
    const empty = await meter.textContent()
    await page
      .getByRole('textbox', { name: 'Message' })
      .fill('A question long enough to count. '.repeat(40))
    await expect(meter).not.toHaveText(empty ?? '')

    // A model loaded with 2K: the instructions alone do not fit.
    await page.getByRole('button', { name: 'Model that answers' }).click()
    const small = page.getByRole('menuitem').filter({ hasText: 'e2e-small' })
    await expect(small).toContainText('Loaded · reads 2K')
    await small.click()
    await expect(meter).toHaveText(/ \/ 2K$/)
    await expect(page.getByRole('alert')).toContainText(
      'no longer fits the model’s context (2K)',
    )
    const sent = chatRequests(model).length
    await page.getByRole('textbox', { name: 'Message' }).press('Enter')
    await expect(
      chatColumn(page).getByText(
        'This conversation doesn’t fit the context the model is loaded with',
      ),
    ).toBeVisible({ timeout: 30_000 })
    await settled(page)
    expect(chatRequests(model).length, 'nothing was sent').toBe(sent)

    // Back to the large one: the next message goes to it.
    await page.getByRole('button', { name: 'Model that answers' }).click()
    await page.getByRole('menuitem').filter({ hasText: 'e2e-large' }).click()
    await expect(meter).toHaveText(/ \/ 32K$/)
    await ask(page, 'Which model answers now?')
    await expect(
      chatColumn(page).getByText('Answered by e2e-large.'),
    ).toBeVisible({ timeout: 60_000 })
    await settled(page)
    expect(chatRequests(model).at(-1)?.body.model).toBe('e2e-large')
  } finally {
    await model.close()
  }
})

test('editing an earlier question answers it again and drops what came after', async ({
  page,
}) => {
  const model = await startFakeModel({
    apiKey: KEY,
    script: (body) =>
      offersTools(body) || body.messages?.length
        ? { content: `Answer to: ${lastUserText(body)}` }
        : { content: 'OK' },
  })
  try {
    await addService(page, { name: 'E2E edit', address: model.url, key: KEY })
    await page.goto('/#/ask')
    for (const question of [
      'First question',
      'Second question',
      'Third question',
    ]) {
      await ask(page, question)
      await expect(
        chatColumn(page).getByText(`Answer to: ${question}`),
      ).toBeVisible({ timeout: 60_000 })
      await settled(page)
    }

    const second = chatColumn(page).getByText('Second question', {
      exact: true,
    })
    await second.hover()
    await second
      .locator('xpath=..')
      .getByRole('button', { name: 'Edit message' })
      .click()
    const editor = page.getByRole('textbox', { name: 'Edited message' })
    await expect(editor).toHaveValue('Second question')
    await expect(
      chatColumn(page).getByText(
        'The answers after this message are replaced by a new one.',
      ),
    ).toBeVisible()
    await editor.fill('Second question, put better')
    await editor.press('Enter')
    await expect(
      chatColumn(page).getByText('Answer to: Second question, put better'),
    ).toBeVisible({ timeout: 60_000 })
    await settled(page)
    for (const gone of [
      'Answer to: Second question',
      'Third question',
      'Answer to: Third question',
    ])
      await expect(
        chatColumn(page).getByText(gone, { exact: true }),
      ).toHaveCount(0)
    // The model was sent the first exchange and the edited question only.
    expect(
      (chatRequests(model).at(-1)?.body.messages ?? [])
        .filter(({ role }) => role !== 'system')
        .map(({ content }) => textOf(content)),
    ).toEqual([
      'First question',
      'Answer to: First question',
      'Second question, put better',
    ])

    // A finished answer can be asked for again.
    const requests = chatRequests(model).length
    await page.getByRole('button', { name: 'Answer again' }).click()
    await settled(page)
    expect(chatRequests(model).length).toBe(requests + 1)
    expect(lastUserText(chatRequests(model).at(-1)!.body)).toBe(
      'Second question, put better',
    )

    // Saved that way: after a reload the chat holds the four messages.
    await page.reload()
    await page
      .getByRole('complementary', { name: 'Conversations' })
      .getByRole('button', { name: /^First question/ })
      .click()
    await expect(
      chatColumn(page).getByText('Answer to: Second question, put better'),
    ).toBeVisible()
    await expect(
      chatColumn(page).getByText('Third question', { exact: true }),
    ).toHaveCount(0)
  } finally {
    await model.close()
  }
})
