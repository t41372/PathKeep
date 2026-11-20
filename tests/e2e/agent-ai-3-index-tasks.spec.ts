/**
 * The Agent conversations' semantic index build, held half way, as
 * Settings › Agent conversations and the background tasks page show it and
 * act on it (`agent-history.md` §7, `data-spaces.md` §5.2). On the backed-up
 * synthetic conversations, with an OpenAI-compatible embeddings service on
 * this computer (`support/fake-embeddings.ts`) that holds every request, so
 * the build stops on its first batch for as long as the test needs, with no
 * timing involved.
 *
 * Proves:
 * - Turning "Build a semantic search index" on starts a build: Settings says
 *   it is being built and that search by meaning waits for it, with a bar and
 *   the passages read so far; the tasks page lists it under Running with the
 *   Agent space's tile, Pause and Cancel, and links it to Settings; the
 *   rail's list names it.
 * - Pause on the tasks page moves it to Waiting with Continue and "You paused
 *   it"; Settings says it is paused and offers Continue, which runs it again,
 *   and the tasks page has it under Running once more.
 * - Cancel on the tasks page ends it: it leaves the page, the backend's job
 *   is cancelled and nothing was published; Settings says it is not built and
 *   that you cancelled the last build, and offers Build now.
 * - Build now (the service answering again) builds it to the end: Settings
 *   says it is ready, with the same passage count the backend serves and the
 *   service's name and model, and the tasks page no longer lists it.
 * - Turning the switch off says plainly that nothing is built and the index
 *   stays on this computer, unused, at the size the backend reports; the
 *   backend reports it off.
 * - Free… asks first, showing that size. When the index changes between the
 *   size being read and the confirmation (here, another preview replaces the
 *   one on screen), nothing is freed: the dialog reads the size again and
 *   asks again. Confirmed then, the backend frees exactly that many bytes,
 *   reports none kept, and the line goes away.
 *
 * The service is a new one (its own id and model), so nothing of the index
 * an earlier spec built is reused and every passage is sent to it. From the
 * hold to Cancel the build waits on one request, which the app gives up on
 * after 120 seconds; the steps between take seconds, and the spec says so
 * if they ever take that long. Runs in
 * the `agent-ai` project after the semantic search and Ask specs; it puts
 * back the AI settings and the switch as it found them.
 */
import { expect, test, type Page } from '@playwright/test'
import type { AiSettings, AppConfig, AppSnapshot } from '../../src/lib/types'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { openTaskList, taskRow } from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import {
  startFakeEmbeddings,
  type FakeEmbeddings,
} from './support/fake-embeddings'
import { backend } from './support/fixture'

test.describe.configure({ mode: 'serial' })

const SERVICE = {
  id: 'e2e-agent-index-tasks',
  name: 'E2E agent index',
  model: 'e2e-embed-tasks',
}
const TASK = 'Index Agent conversations for semantic search'
const SWITCH = 'Build a semantic search index'

interface IndexStatus {
  enabled: boolean
  keptBytes: string
  serving: { provider: string; model: string; passages: number } | null
  stale?: boolean
  job: { jobId: string; state: string; passages: number } | null
}

const status = () =>
  backend<IndexStatus>('get_agent_semantic_index', { request: {} })
const snapshot = () => backend<AppSnapshot>('app_snapshot')
const setSwitch = (on: boolean) =>
  backend('set_agent_viewer_preferences', {
    request: { buildSemanticIndex: on },
  })

let service: FakeEmbeddings | null = null
let before: { ai: AiSettings; buildSemanticIndex: boolean } | null = null
const evidence: unknown[] = []

async function openSettings(page: Page) {
  await page.goto('/#/settings/agent')
  await unlockIfLocked(page)
  await expect(page.getByRole('switch', { name: SWITCH })).toBeVisible({
    timeout: 60_000,
  })
}

async function openTasks(page: Page) {
  await page.goto('/#/tasks')
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Background tasks', level: 1 }),
  ).toBeVisible()
}

const indexStatus = (page: Page) =>
  page.getByRole('status', { name: 'Semantic search index' })

const label = (action: 'Pause' | 'Continue' | 'Cancel') => `${action} “${TASK}”`

test.beforeAll(async ({ browser }) => {
  service = await startFakeEmbeddings({
    holdAfter: 0,
    models: [
      {
        id: SERVICE.model,
        type: 'embeddings',
        state: 'loaded',
        max_context_length: 2048,
        loaded_context_length: 2048,
      },
    ],
  })
  const page = await browser.newPage({
    baseURL: resolveDesktopBridgeEnv(process.env).devServerUrl,
  })
  try {
    await openSettings(page)
    const { config } = await snapshot()
    const prefs = await backend<{ buildSemanticIndex: boolean }>(
      'get_agent_viewer_preferences',
      { request: {} },
    )
    before = { ai: config.ai, buildSemanticIndex: prefs.buildSemanticIndex }
    // Off first, so turning it on in the test is what starts the build.
    await setSwitch(false)
    await backend('save_config', {
      config: {
        ...config,
        ai: {
          ...config.ai,
          enabled: true,
          semanticIndexEnabled: true,
          autoIndexAfterBackup: false,
          embeddingProviders: [
            ...config.ai.embeddingProviders.filter(
              (provider) => provider.id !== SERVICE.id,
            ),
            {
              id: SERVICE.id,
              name: SERVICE.name,
              purpose: 'embedding',
              requestFormat: 'openai',
              enabled: true,
              baseUrl: service.url,
              apiKeySaved: false,
              defaultModel: SERVICE.model,
              modelCatalog: [],
            },
          ],
          embeddingProviderId: SERVICE.id,
        },
      } satisfies AppConfig,
    })
  } finally {
    await page.close()
  }
})

test.afterAll(async ({ browser }) => {
  service?.release()
  const page = await browser.newPage({
    baseURL: resolveDesktopBridgeEnv(process.env).devServerUrl,
  })
  try {
    await openSettings(page)
    if (before) {
      await setSwitch(before.buildSemanticIndex)
      const { config } = await snapshot()
      await backend('save_config', {
        config: { ...config, ai: before.ai } satisfies AppConfig,
      })
    }
  } finally {
    await page.close()
    await service?.close()
  }
})

test('a held build shows in Settings and on the tasks page; paused, continued and cancelled there, then built to the end', async ({
  page,
}, info) => {
  test.setTimeout(300_000)
  await openSettings(page)
  const toggle = page.getByRole('switch', { name: SWITCH })
  await expect(toggle).not.toBeChecked()
  await expect(
    page.getByText(/^Off: nothing is built and search doesn’t use the index\./),
  ).toBeVisible()

  // On: the build starts and its first request to the service is held.
  await toggle.click()
  await expect(toggle).toBeChecked()
  await expect(indexStatus(page)).toContainText('Building', {
    timeout: 60_000,
  })
  await expect(indexStatus(page)).toContainText(
    'Semantic search can be used once it’s done.',
  )
  await expect
    .poll(() => service!.held(), { timeout: 60_000 })
    .toBeGreaterThan(0)
  const heldAt = Date.now()
  await expect(
    page.getByRole('progressbar', { name: 'Semantic search index' }),
  ).toBeVisible()
  await expect(page.getByText(/passages? read so far$/)).toBeVisible()
  await expect(
    page.getByText(/^Turning this off stops any build/),
  ).toBeVisible()
  const started = await status()
  evidence.push({ step: 'held', status: started })
  expect(started.enabled).toBe(true)
  expect(started.serving).toBeNull()
  expect(started.job?.state).toBe('running')
  const jobId = started.job!.jobId
  const row = `li[data-task-id="${jobId}"]`

  // The tasks page: under Running, with the Agent space's tile.
  await openTasks(page)
  const running = await taskRow(page, 'running', row)
  await expect(running).toHaveAttribute('data-kind', 'agent-history')
  await expect(running).toContainText(TASK)
  await expect(running).toContainText(/passages? read so far/)
  await expect(
    running.getByText('Agent conversations', { exact: true }),
  ).toBeAttached()
  await expect(running.getByRole('link', { name: TASK })).toHaveAttribute(
    'href',
    '#/settings/agent',
  )
  await expect(running.getByRole('progressbar', { name: TASK })).toBeVisible()
  await expect(
    running.getByRole('button', { name: label('Cancel') }),
  ).toBeEnabled()
  const list = await openTaskList(page)
  await expect(list).toContainText(TASK)
  await page.keyboard.press('Escape')

  // Pause: it waits, says you paused it, and offers Continue.
  await running.getByRole('button', { name: label('Pause') }).click()
  const waiting = await taskRow(page, 'waiting', row)
  await expect(waiting).toContainText('You paused it')
  await expect(
    waiting.getByRole('button', { name: label('Continue') }),
  ).toBeEnabled()
  await expect(
    page.locator(`section[data-task-group="running"] ${row}`),
  ).toHaveCount(0)
  expect((await status()).job?.state).toBe('paused')

  // Settings says the same, and Continue there runs it again.
  await openSettings(page)
  await expect(indexStatus(page)).toContainText('Paused')
  await expect(indexStatus(page)).toContainText(
    'Nothing is built while it’s paused.',
  )
  await page.getByRole('button', { name: label('Continue') }).click()
  await expect(indexStatus(page)).toContainText('Building')
  await expect(page.getByRole('button', { name: label('Pause') })).toBeEnabled()
  expect((await status()).job?.state).toBe('running')

  // Cancel on the tasks page: it ends, and nothing is published.
  await openTasks(page)
  const again = await taskRow(page, 'running', row)
  // Everything since the hold is one embeddings request; the app gives up on
  // one after 120 seconds, and the build would fail instead of being cancelled.
  expect(
    Date.now() - heldAt,
    'the steps took as long as the app waits for an embeddings reply',
  ).toBeLessThan(110_000)
  await again.getByRole('button', { name: label('Cancel') }).click()
  await expect(page.locator(row)).toHaveCount(0, { timeout: 60_000 })
  await expect
    .poll(async () => (await status()).job?.state, { timeout: 60_000 })
    .toBe('cancelled')
  const cancelled = await status()
  evidence.push({ step: 'cancelled', status: cancelled })
  expect(cancelled.serving).toBeNull()

  // Settings: not built, you cancelled it, Build now.
  await openSettings(page)
  await expect(indexStatus(page)).toContainText('Not built yet')
  await expect(indexStatus(page)).toContainText('You cancelled the last build.')
  service!.release()
  await page.getByRole('button', { name: 'Build now' }).click()
  await expect(indexStatus(page)).toContainText('Ready', {
    timeout: 240_000,
  })
  const built = await status()
  evidence.push({ step: 'built', status: built })
  expect(built.serving?.provider).toBe(SERVICE.id)
  expect(built.serving?.model).toBe(SERVICE.model)
  expect(built.stale).toBe(false)
  expect(built.job?.state).toBe('ready')
  const passages = built.serving!.passages
  expect(passages).toBeGreaterThan(0)
  // "just now", or a minute or two on a slow machine.
  await expect(indexStatus(page)).toContainText(
    new RegExp(
      `${passages.toLocaleString('en-US')} passages?, built (just now|\\d+ min ago) with ${SERVICE.name} · ${SERVICE.model}\\.`,
    ),
  )
  expect(service!.requestsFor(SERVICE.model)).toBeGreaterThan(0)

  // Nothing is left on the tasks page.
  await openTasks(page)
  await expect(page.getByText(TASK)).toHaveCount(0)

  // Off: plainly kept and unused, at the size the backend reports.
  await openSettings(page)
  await page.getByRole('switch', { name: SWITCH }).click()
  await expect(page.getByRole('switch', { name: SWITCH })).not.toBeChecked()
  await expect(indexStatus(page)).toHaveCount(0)
  await expect(
    page.getByText('Off: nothing is built and search doesn’t use the index.', {
      exact: true,
    }),
  ).toBeVisible()
  await expect.poll(async () => (await status()).enabled).toBe(false)
  const off = await status()
  evidence.push({ step: 'off', status: off })
  expect(Number(off.keptBytes), 'the index is kept').toBeGreaterThan(0)
  const keptLine = page.locator('[data-kept-bytes]')
  await expect(keptLine).toHaveAttribute('data-kept-bytes', off.keptBytes)
  const lineText = await keptLine.innerText()
  const size = /unused: (.+)\.$/.exec(lineText)?.[1]
  expect(size, `a size in “${lineText}”`).toBeTruthy()

  // Free…: the size again in the confirmation.
  await page
    .getByRole('button', {
      name: 'Free the semantic search index kept on this computer',
    })
    .click()
  const dialog = page.getByRole('alertdialog', {
    name: 'Free the semantic search index?',
  })
  await expect(dialog).toContainText(
    `This deletes the index kept on this computer: ${size}.`,
  )
  const confirm = dialog.getByRole('button', { name: `Free ${size}` })
  await expect(confirm).toBeVisible()

  // Another preview replaces the one on screen: confirming it frees nothing,
  // and the dialog reads the size again and asks again.
  await backend('preview_free_agent_semantic_index', { request: {} })
  const refused = page.waitForResponse((response) =>
    response.url().endsWith('/commands/free_agent_semantic_index'),
  )
  await confirm.click()
  const refusal = await refused
  expect(refusal.ok(), 'the replaced preview is refused').toBe(false)
  expect(((await refusal.json()) as { code?: string }).code).toBe(
    'preview-expired',
  )
  await expect(dialog).toContainText(
    'The index changed since its size was read. Here is its size now.',
  )
  expect((await status()).keptBytes, 'nothing freed yet').toBe(off.keptBytes)
  const freed = page.waitForResponse(
    (response) =>
      response.url().endsWith('/commands/free_agent_semantic_index') &&
      response.ok(),
  )
  await dialog.getByRole('button', { name: `Free ${size}` }).click()
  const result = (await (await freed).json()) as unknown
  evidence.push({ step: 'freed', result })
  expect(JSON.stringify(result)).toContain(`"freedBytes":"${off.keptBytes}"`)
  await expect(dialog).toHaveCount(0)
  await expect(
    page.locator('[data-sonner-toast]').filter({ hasText: `Freed ${size}` }),
  ).toBeVisible()
  await expect(keptLine).toHaveCount(0)
  const after = await status()
  evidence.push({ step: 'after free', status: after })
  expect(after.keptBytes).toBe('0')
  expect(after.serving).toBeNull()

  await info.attach('index build evidence', {
    body: JSON.stringify({ evidence, requests: service!.requests() }, null, 2),
    contentType: 'application/json',
  })
  await info.attach('settings after', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})
