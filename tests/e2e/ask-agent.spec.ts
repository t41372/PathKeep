/**
 * What Ask sends the model and how the conversation can be steered, against
 * scripted services on this computer (`support/fake-model.ts`). Whether a
 * real model answers better with it is tried by hand against LM Studio; this
 * proves what reaches the model and what the person sees.
 *
 * Proves:
 * - The model gets one system message, first, with the rules (reply in the
 *   language of the latest message, choose when the choice is left to it)
 *   and the user's own instructions from Settings after them; no request
 *   carries sampling settings or a cap on the answer, so the model's own
 *   settings apply. What the model writes before it calls a
 *   tool is shown as thinking, and only its last turn as the answer, under
 *   the model that wrote it.
 * - With LM Studio, the composer shows how much of the loaded model's
 *   context the next message takes. A model loaded with too small a context
 *   turns it red with a plain warning, and the message is not sent: the
 *   answer says why. Picking another model beside the composer sends the
 *   next message to that one.
 * - Editing an earlier question answers it again and drops what came after,
 *   also once saved; a finished answer can be asked for again.
 * - A turn that breaks off half way (LM Studio's error inside the stream, as
 *   on 2026-10-15) runs none of its calls; the next request still holds the
 *   question and says what failed. Arguments sent as a JSON string are read
 *   once and go back to the model as an object; arguments that cannot be
 *   read go back as `{}`, with the error and the path in the tool result.
 *   The model's thinking goes back with its turn in the same run.
 * - A model that searches and then ends its turn without writing anything
 *   (only thinking) is said to have stopped without an answer: the turn
 *   keeps the pages it found and can be asked again.
 * - Every time and day the tools give the model carries its weekday, worked
 *   out on this computer: search rows read `2026-10-15 Thu 14:02`, report
 *   days have a `weekday` beside them, and sandbox rows keep
 *   `visitedAtLocal` as `YYYY-MM-DD HH:MM` with `localWeekday` beside it.
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
import { unlockIfLocked } from './support/app'
import { archivedVisits, backend, localDay } from './support/fixture'

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
    // Only what the request needs: the model's own sampling and length apply.
    for (const key of [
      'temperature',
      'top_p',
      'top_k',
      'max_tokens',
      'max_completion_tokens',
    ])
      expect(first.body, key).not.toHaveProperty(key)
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

test('a turn that breaks off runs none of its calls, and the question stays in front of the model', async ({
  page,
}, testInfo) => {
  const question = '這週我讀過哪些講 spawn_blocking 的頁面？'
  const thinking = 'Search the archive for spawn_blocking.'
  const answer = '你這週讀的是 spawn_blocking 用完執行緒的那篇。'
  const failed = (body: FakeRequest['body']) =>
    lastUserText(body).includes('None of its tool calls were executed')
  const model = await startFakeModel({
    apiKey: KEY,
    script: (body) => {
      if (!offersTools(body)) return { content: 'OK' }
      const results = toolResults(body)
      if (results.length === 0 && !failed(body))
        return {
          reasoning: 'Get the overview of this week first.',
          toolCalls: [
            {
              name: 'intelligence_report',
              arguments: { report: 'overview', start_date: '2026-10-12' },
            },
          ],
          breakOff: 'model repeated a tool parameter',
        }
      if (results.length === 0)
        return {
          reasoning: thinking,
          toolCalls: [
            {
              name: 'search_history',
              // The arguments as a JSON string, once more encoded.
              arguments: JSON.stringify(
                JSON.stringify({ query: 'spawn_blocking' }),
              ),
            },
          ],
        }
      if (results.length === 1)
        return {
          toolCalls: [{ name: 'search_history', arguments: { limit: 'lots' } }],
        }
      return { content: answer }
    },
  })
  try {
    await addService(page, { name: 'E2E turns', address: model.url, key: KEY })
    await page.goto('/#/ask')
    await ask(page, question)
    await expect(chatColumn(page).getByText(answer)).toBeVisible({
      timeout: 60_000,
    })
    await settled(page)

    const requests = chatRequests(model)
      .map(({ body }) => body)
      .filter(offersTools)
    await testInfo.attach('requests', {
      body: JSON.stringify(requests, null, 2),
      contentType: 'application/json',
    })
    expect(requests).toHaveLength(4)
    const [, retry, afterSearch, afterError] = requests
    type Wire = {
      role: string
      reasoning_content?: string
      tool_calls?: { function?: { arguments?: string } }[]
    }
    const wire = (body: FakeRequest['body']) =>
      (body.messages ?? []) as unknown as Wire[]

    // The broken turn: nothing ran, the question is still there, and why,
    // in a paragraph of its own (rig joins two user messages in a row).
    expect(toolResults(retry), 'the broken call did not run').toEqual([])
    const users = (retry.messages ?? [])
      .filter((message) => message.role === 'user')
      .map((message) => textOf(message.content))
      .join('')
    expect(users.startsWith(question), users).toBe(true)
    expect(users).toContain(`${question}\n\nThe previous model turn failed: `)
    expect(users).toContain('model repeated a tool parameter')

    // A JSON string read once: the search ran, and went back as an object.
    const searched = wire(afterSearch).findLast(
      (message) => message.role === 'assistant',
    )!
    expect(
      JSON.parse(searched.tool_calls?.[0].function?.arguments ?? 'null'),
    ).toEqual({ query: 'spawn_blocking' })
    expect(searched.reasoning_content, 'thinking goes back').toBe(thinking)
    expect(toolResults(afterSearch)[0]).toContain('spawn_blocking')

    // Arguments that cannot be read: `{}` back, the error and its path in the result.
    const broken = wire(afterError).findLast(
      (message) => message.role === 'assistant',
    )!
    expect(broken.tool_calls?.[0].function?.arguments).toBe('{}')
    const error = toolResults(afterError)[1]
    expect(error).toContain('$["limit"]')
    expect(error).toContain('"lots"')
    for (const body of requests)
      for (const key of ['temperature', 'max_tokens'])
        expect(body, key).not.toHaveProperty(key)
  } finally {
    await model.close()
  }
})

test('every time and day the tools give the model carries its weekday', async ({
  page,
}, testInfo) => {
  const title = 'spawn_blocking pool exhaustion · tokio-rs/tokio'
  const visits = archivedVisits().filter((visit) => visit.title === title)
  expect(visits.length, 'the fixture has the page').toBeGreaterThan(0)
  const newest = Math.max(...visits.map((visit) => visit.at))
  // The app and this test run in the same zone (UTC, from run-playwright).
  const weekday = (at: number) =>
    new Intl.DateTimeFormat('en-US', { weekday: 'short' }).format(at)
  const clock = (at: number) => new Date(at).toTimeString().slice(0, 5)
  const shown = new Set(
    visits.map(
      (visit) =>
        `${localDay(visit.at)} ${weekday(visit.at)} ${clock(visit.at)}`,
    ),
  )
  const day = localDay(newest)
  const answer = 'Done.'
  const model = await startFakeModel({
    apiKey: KEY,
    script: (body) => {
      if (!offersTools(body)) return { content: 'OK' }
      const step = toolResults(body).length
      if (step === 0)
        return {
          toolCalls: [
            { name: 'search_history', arguments: { query: 'spawn_blocking' } },
          ],
        }
      if (step === 1)
        return {
          toolCalls: [
            {
              name: 'intelligence_report',
              arguments: { report: 'day_insights', start_date: day },
            },
          ],
        }
      if (step === 2)
        return {
          toolCalls: [
            {
              name: 'run_code',
              arguments: {
                source:
                  "return query_history({ query: 'spawn_blocking', limit: 20 }).rows.map((row) => [row.visitedAtLocal, row.localDate, row.localWeekday])",
              },
            },
          ],
        }
      return { content: answer }
    },
  })
  try {
    await addService(page, { name: 'E2E days', address: model.url, key: KEY })
    await page.goto('/#/ask')
    await ask(page, 'What did I read on spawn_blocking, and on which day?')
    await expect(chatColumn(page).getByText(answer)).toBeVisible({
      timeout: 60_000,
    })
    await settled(page)
    const last = chatRequests(model)
      .map(({ body }) => body)
      .filter(offersTools)
      .at(-1)!
    const [search, report, sandbox] = toolResults(last)
    await testInfo.attach('tool results', {
      body: JSON.stringify({ search, report, sandbox }, null, 2),
      contentType: 'application/json',
    })

    // The search row for the page: a time this test computes from a visit.
    const row = search.split('\n').find((line) => line.includes(title))
    expect(row, 'the page is in the search results').toBeTruthy()
    const time = /^\[\d+\] (.+?) \|/.exec(row!)?.[1]
    expect(
      shown,
      `${time} is one of the page's visits, with its weekday`,
    ).toContain(time)

    // The report reaches the model as pages of JSON text: every day in the
    // first page, the requested one among them, has its weekday on the next line.
    const fragment = (
      JSON.parse(report.slice(report.indexOf('\n') + 1)) as {
        jsonFragment: string
      }
    ).jsonFragment
    const dated = [
      ...fragment.matchAll(/"(?:date|dateKey)": "(\d{4}-\d{2}-\d{2})"/g),
    ].filter((match) => match.index < fragment.length - 64)
    const days = [
      ...fragment.matchAll(
        /"(?:date|dateKey)": "(\d{4}-\d{2}-\d{2})",\n\s*"weekday": "(\w+)"/g,
      ),
    ]
    expect(days.map(([, date]) => date)).toContain(day)
    expect(days.length, 'no day without its weekday').toBe(dated.length)
    for (const [, date, name] of days)
      expect(name, date).toBe(weekday(Date.parse(`${date}T12:00:00Z`)))

    // Sandbox rows: the time keeps its shape, the weekday has a field.
    const rows = JSON.parse(
      /\[\s*\[[\s\S]*\]\s*\]/.exec(sandbox)?.[0] ?? '[]',
    ) as [string, string, string][]
    expect(rows.length, 'the sandbox found the page').toBeGreaterThan(0)
    for (const [local, date, weekdayName] of rows) {
      expect(local).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/)
      expect(local.startsWith(date)).toBe(true)
      expect(weekdayName, date).toBe(weekday(Date.parse(`${date}T12:00:00Z`)))
    }
  } finally {
    await model.close()
  }
})

test('a model that stops without writing an answer is said so, and the pages it found stay', async ({
  page,
}) => {
  const issue = 'spawn_blocking pool exhaustion · tokio-rs/tokio'
  const model = await startFakeModel({
    apiKey: KEY,
    script: (body) => {
      if (!offersTools(body)) return { content: 'OK' }
      // Some models end on their thinking alone once they have results.
      if (body.messages?.at(-1)?.role === 'tool')
        return { reasoning: 'The issue about spawn_blocking is the one.' }
      return {
        toolCalls: [
          { name: 'search_history', arguments: { query: 'spawn_blocking' } },
        ],
      }
    },
  })
  try {
    await addService(page, {
      name: 'E2E silent',
      address: model.url,
      key: KEY,
    })
    await page.goto('/#/ask')
    await unlockIfLocked(page)
    await ask(page, 'Where did I read about spawn_blocking?')
    await expect(
      chatColumn(page).getByText(
        'The model stopped without writing an answer. Ask again, or pick another model beside the message box.',
      ),
    ).toBeVisible({ timeout: 60_000 })
    await settled(page)
    expect(
      chatRequests(model).some(
        (request) => toolResults(request.body).length > 0,
      ),
      'the search ran',
    ).toBe(true)
    await expect(
      chatColumn(page)
        .getByLabel('Pages used')
        .getByRole('link', { name: issue, exact: false }),
    ).toBeVisible()
    // Nothing to copy, and the question can be asked again.
    await expect(
      chatColumn(page).getByRole('button', { name: 'Copy answer' }),
    ).toHaveCount(0)
    await expect(
      chatColumn(page).getByRole('button', { name: 'Answer again' }),
    ).toBeVisible()
  } finally {
    await model.close()
  }
})
