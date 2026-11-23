/**
 * Freeing browsing history's semantic search index, which stays on disk when
 * semantic search is off (`intelligence.md`, "Free the browsing semantic
 * index" in `desktop-command-surface.md`). The index is built for real, with
 * an OpenAI-compatible embeddings service on this computer
 * (`support/fake-embeddings.ts`).
 *
 * Proves:
 * - With semantic search on and an index built, choosing another model asks
 *   first, and the confirmation says both indexes are on disk while the new
 *   one is built and how much the one there now takes: the size the backend
 *   reports. Cancel keeps the model, in the picker and in the saved settings.
 * - Free up space lists the index with that size, but can't tick it while
 *   semantic search is on, and says to turn it off in Settings › AI first.
 * - Off, Settings › AI says the index is still on this computer, unused, at
 *   the size the backend reports. Free… asks first with that size, what a
 *   later build costs and what stays. When the index changes between the size
 *   being read and the confirmation (here, another preview replaces the one
 *   on screen), nothing is freed: the dialog reads the size again and asks
 *   again. Confirmed then, the backend frees exactly that many bytes, the line
 *   goes, the backend has no index left, the switch prompt no longer speaks
 *   of disk space, and Storage's search indexes figure drops.
 * - Built again and off again, Free up space frees the index together with
 *   another ticked row (a leftover file in the temporary files) in one
 *   confirmation: the index first, since a prune records its run in the
 *   archive and so would expire the index's preview, then the rest. The
 *   notice gives the size on the button, and both are gone.
 *
 * Runs in its own project after `agent-ai`, whose first spec builds a
 * browsing index too. It puts back the AI settings it found and leaves no
 * browsing index on disk; no spec after it uses one.
 */
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import type { AiSettings, AppConfig, AppSnapshot } from '../../src/lib/types'
import { addEmbeddingsService } from './support/ai-service'
import { unlockIfLocked } from './support/app'
import {
  startFakeEmbeddings,
  type FakeEmbeddings,
} from './support/fake-embeddings'
import { backend, fixture } from './support/fixture'

test.describe.configure({ mode: 'serial' })

const SERVICE = { name: 'Free-space embeddings', model: 'e2e-free-embed' }
const DIMENSIONS = 384
/** A file a backup or an import could have left: 256 KiB, in the temporary files. */
const LEFTOVER_BYTES = 256 * 1024
const leftover = () =>
  path.join(fixture().dirs.projectRoot, 'staging', 'e2e-semantic-free.bin')

interface QueueStatus {
  running: number
  queued: number
  semanticIndex: {
    chosen: { providerId: string; model: string } | null
    serving: { providerId: string; model: string } | null
    chosenComplete: boolean
    build: unknown
  }
}
const queueStatus = () => backend<QueueStatus>('load_ai_queue_status')
const snapshot = () => backend<AppSnapshot>('app_snapshot')
const indexBytes = async () => (await snapshot()).aiStatus.semanticSidecarBytes
const storageIndexBytes = async () =>
  (
    await backend<{ storage: { semanticSidecarBytes: number } }>(
      'load_dashboard_snapshot',
    )
  ).storage.semanticSidecarBytes

let service: FakeEmbeddings | null = null
let before: AiSettings | null = null
const evidence: Record<string, unknown>[] = []

test.beforeAll(async () => {
  service = await startFakeEmbeddings({
    // Big enough vectors that the index is a visible part of Storage's
    // rounded search indexes figure.
    dimensions: DIMENSIONS,
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
})

test.afterAll(async ({ browser }, info) => {
  info.setTimeout(240_000)
  rmSync(leftover(), { force: true })
  try {
    if (!before) return
    const page = await browser.newPage({
      baseURL: resolveDesktopBridgeEnv(process.env).devServerUrl,
    })
    try {
      await page.goto('/#/settings/ai')
      await unlockIfLocked(page)
      // No build of this spec's left running against a service about to close.
      await expect
        .poll(async () => (await queueStatus()).running, { timeout: 120_000 })
        .toBe(0)
      const { config } = await snapshot()
      await backend('save_config', {
        config: { ...config, ai: before } satisfies AppConfig,
      })
    } finally {
      await page.close()
    }
  } finally {
    await service?.close()
  }
})

/** What each step read from the backend, kept with the report. */
async function attachEvidence(info: TestInfo) {
  await info.attach('free evidence', {
    body: JSON.stringify(evidence, null, 2),
    contentType: 'application/json',
  })
}

async function openAiSettings(page: Page) {
  await page.goto('/#/settings/ai')
  // A load reads the settings and the index's size as they are now.
  await page.reload()
  await unlockIfLocked(page)
  await expect(semanticSwitch(page)).toBeVisible({ timeout: 60_000 })
}

const semanticSwitch = (page: Page) =>
  page.getByRole('switch', { name: 'Semantic search' })

/** Builds the chosen service's index with semantic search on, to the end. */
async function buildIndex(page: Page) {
  if (!(await semanticSwitch(page).isChecked()))
    await semanticSwitch(page).click()
  else
    await page.getByRole('button', { name: 'Build index', exact: true }).click()
  await expect(semanticSwitch(page)).toBeChecked()
  await expect
    .poll(
      async () => {
        const queue = await queueStatus()
        return (
          queue.semanticIndex.chosenComplete &&
          queue.semanticIndex.build === null &&
          queue.running === 0 &&
          queue.queued === 0
        )
      },
      { timeout: 240_000, intervals: [500, 1_000, 2_000] },
    )
    .toBe(true)
  expect(await indexBytes(), 'the index is on disk').toBeGreaterThan(0)
}

/** Turns semantic search off and waits for the index work to be idle. */
async function turnOff(page: Page) {
  await semanticSwitch(page).click()
  await expect(semanticSwitch(page)).not.toBeChecked()
  await expect
    .poll(async () => (await snapshot()).config.ai.semanticIndexEnabled)
    .toBe(false)
  await expect
    .poll(async () => {
      const queue = await queueStatus()
      return queue.running + queue.queued
    })
    .toBe(0)
}

async function openFreeUpSpace(page: Page) {
  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  await page
    .getByText('Free up space', { exact: true })
    .locator('xpath=../..')
    .getByRole('button', { name: 'Review…' })
    .click()
  const dialog = page.getByRole('dialog', { name: 'Free up space' })
  await expect(dialog).toBeVisible()
  return dialog
}

/**
 * Storage's disk usage figure for search indexes (the index files and the
 * search and Intelligence databases), in bytes as shown, so rounded.
 */
async function shownSearchIndexes(page: Page) {
  const text = (await page.getByText(/^Search indexes /).innerText()).trim()
  const match = /([\d.,]+) (B|KB|MB|GB|TB)$/.exec(text)
  expect(match, `a size in “${text}”`).toBeTruthy()
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  return Number(match![1].replace(/,/g, '')) * 1024 ** units.indexOf(match![2])
}

test('with an index built, the model switch says what it takes on disk and Cancel keeps the model; Free up space can’t free it while semantic search is on', async ({
  page,
}, info) => {
  test.setTimeout(420_000)
  await openAiSettings(page)
  before = (await snapshot()).config.ai
  // Index work another spec left must not be the one waited for below.
  await expect
    .poll(async () => (await queueStatus()).running, { timeout: 120_000 })
    .toBe(0)

  await addEmbeddingsService(page, service!.url, SERVICE.model, SERVICE.name)
  await buildIndex(page)
  const chosen = (await snapshot()).config.ai.embeddingProviderId
  expect(chosen).toBeTruthy()

  // Choosing another model asks first, with the size the backend reports.
  await openAiSettings(page)
  const bytes = await indexBytes()
  evidence.push({ step: 'built', bytes, queue: await queueStatus() })
  const picker = page.getByRole('combobox', { name: 'Model' })
  await expect(picker).toContainText(SERVICE.name)
  await picker.click()
  await page.getByRole('option', { name: 'In-app static model' }).click()
  const confirm = page.getByRole('alertdialog', {
    name: 'Switch semantic search to the in-app static model?',
  })
  const disk = confirm.locator('[data-index-bytes]')
  await expect(disk).toHaveAttribute('data-index-bytes', String(bytes))
  const diskText = await disk.innerText()
  const size =
    /^While it is built, both indexes are on this computer; the one you have now takes (.+)\.$/.exec(
      diskText,
    )?.[1]
  expect(size, `a size in “${diskText}”`).toBeTruthy()
  // Said beside what the switch already said: the old index goes once the
  // new one is complete, and nothing starts before Build index.
  await expect(confirm).toContainText(
    `Once it is complete, the index made with ${SERVICE.model} is deleted.`,
  )
  await expect(confirm).toContainText('Nothing starts before that.')
  await info.attach('switch prompt', {
    body: await confirm.screenshot(),
    contentType: 'image/png',
  })
  await confirm.getByRole('button', { name: 'Cancel' }).click()
  await expect(confirm).toBeHidden()
  await expect(picker).toContainText(SERVICE.name)
  expect((await snapshot()).config.ai.embeddingProviderId).toBe(chosen)
  expect((await queueStatus()).semanticIndex.chosenComplete).toBe(true)

  // Free up space shows it, and says why it can't be ticked.
  const dialog = await openFreeUpSpace(page)
  const indexBox = dialog.getByRole('checkbox', {
    name: /^Semantic search index/,
  })
  await expect(indexBox).toBeDisabled()
  await expect(indexBox).not.toBeChecked()
  const row = dialog.getByRole('listitem').filter({ has: indexBox })
  await expect(row).toContainText(size!)
  await expect(row).toContainText(
    'Turn off semantic search in Settings › AI first.',
  )
  await info.attach('free up space while on', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await expect(dialog).toBeHidden()
})

test('off, Settings › AI frees the index after showing its size, and asks again when it changed', async ({
  page,
}, info) => {
  test.setTimeout(240_000)
  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  const shownBefore = await shownSearchIndexes(page)
  const storedBefore = await storageIndexBytes()
  expect(storedBefore).toBeGreaterThan(0)

  await openAiSettings(page)
  await turnOff(page)
  const kept = await indexBytes()
  expect(kept, 'off keeps the index').toBeGreaterThan(0)
  const keptLine = page.locator('[data-kept-bytes]')
  await expect(keptLine).toHaveAttribute('data-kept-bytes', String(kept))
  const lineText = await keptLine.innerText()
  const size = /^The index is still on this computer, unused: (.+)\.$/.exec(
    lineText,
  )?.[1]
  expect(size, `a size in “${lineText}”`).toBeTruthy()

  await page
    .getByRole('button', {
      name: 'Free the semantic search index kept on this computer',
    })
    .click()
  const dialog = page.getByRole('alertdialog', {
    name: 'Free the semantic search index?',
  })
  await expect(dialog).toContainText(
    `This deletes the semantic search index from this computer, for every model it was built with: ${size}.`,
  )
  await expect(dialog).toContainText(
    'If you turn semantic search on later, the index is built again from the start, which can take a long time on a big archive.',
  )
  await expect(dialog).toContainText(
    'Your visits, full-text search and saved page text stay.',
  )
  const confirm = dialog.getByRole('button', { name: `Free ${size}` })
  await expect(confirm).toBeVisible()
  await info.attach('free confirmation', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })

  // Another preview replaces the one on screen: confirming it frees nothing,
  // and the dialog reads the size again and asks again.
  await backend('preview_free_semantic_index', { request: {} })
  const refused = page.waitForResponse((response) =>
    response.url().endsWith('/commands/free_semantic_index'),
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
  expect(await indexBytes(), 'nothing freed yet').toBe(kept)

  const freed = page.waitForResponse(
    (response) =>
      response.url().endsWith('/commands/free_semantic_index') && response.ok(),
  )
  await dialog.getByRole('button', { name: `Free ${size}` }).click()
  const result = (await (await freed).json()) as {
    freedBytes: string
    keptBytes: string
  }
  evidence.push({ step: 'freed in Settings › AI', kept, result })
  expect(result).toEqual({ freedBytes: String(kept), keptBytes: '0' })
  await expect(dialog).toHaveCount(0)
  await expect(
    page.locator('[data-sonner-toast]').filter({ hasText: `Freed ${size}` }),
  ).toBeVisible()
  await expect(keptLine).toHaveCount(0)

  // No index is left: the backend has none, and the switch prompt has no
  // disk space to speak of.
  expect(await indexBytes()).toBe(0)
  expect((await queueStatus()).semanticIndex.serving).toBeNull()
  const picker = page.getByRole('combobox', { name: 'Model' })
  await picker.click()
  await page.getByRole('option', { name: 'In-app static model' }).click()
  const switching = page.getByRole('alertdialog', {
    name: 'Switch semantic search to the in-app static model?',
  })
  await expect(switching).toContainText('needs its own index')
  await expect(switching.locator('[data-index-bytes]')).toHaveCount(0)
  await switching.getByRole('button', { name: 'Cancel' }).click()
  await expect(picker).toContainText(SERVICE.name)

  // Storage counts it gone, and its disk usage figure drops.
  expect(await storageIndexBytes()).toBe(0)
  await page.goto('/#/settings/storage')
  await unlockIfLocked(page)
  await expect.poll(() => shownSearchIndexes(page)).toBeLessThan(shownBefore)
  evidence.push({
    step: 'storage',
    storedBefore,
    shownBefore,
    shownAfter: await shownSearchIndexes(page),
  })
  await attachEvidence(info)
  await info.attach('storage after', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  })
})

test('Free up space frees the index and another ticked row in one go, the index first', async ({
  page,
}, info) => {
  test.setTimeout(420_000)
  await openAiSettings(page)
  // Built again from the start, then off again.
  await buildIndex(page)
  await turnOff(page)
  const kept = await indexBytes()
  expect(kept).toBeGreaterThan(0)
  mkdirSync(path.dirname(leftover()), { recursive: true })
  writeFileSync(leftover(), Buffer.alloc(LEFTOVER_BYTES, 7))

  const commands: string[] = []
  page.on('request', (request) => {
    const command = /\/commands\/(free_semantic_index|run_retention_prune)$/
      .exec(request.url())
      ?.at(1)
    if (command) commands.push(command)
  })
  const dialog = await openFreeUpSpace(page)
  const indexBox = dialog.getByRole('checkbox', {
    name: /^Semantic search index/,
  })
  // Opt-in, and tickable once the dialog has read its size.
  await expect(indexBox).toBeEnabled()
  await expect(indexBox).not.toBeChecked()
  // Only the temporary files and the index: untick what is ticked by default.
  const ticked = dialog.getByRole('checkbox', { checked: true })
  while ((await ticked.count()) > 0) await ticked.first().click()
  const temporary = dialog.getByRole('checkbox', { name: /^Temporary files/ })
  await temporary.click()
  await indexBox.click()
  await expect(indexBox).toBeChecked()
  const button = dialog.getByRole('button', { name: /^Delete / })
  const total = /^Delete (.+)$/.exec(await button.innerText())?.[1]
  expect(total).toBeTruthy()
  await info.attach('free up space with the index', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })

  await button.click()
  await expect(page.getByText(`Freed ${total}.`)).toBeVisible({
    timeout: 60_000,
  })
  await expect(dialog).toBeHidden()
  evidence.push({ step: 'freed in Free up space', kept, total, commands })
  expect(commands).toEqual(['free_semantic_index', 'run_retention_prune'])
  expect(existsSync(leftover()), 'the leftover is deleted').toBe(false)
  expect(await indexBytes()).toBe(0)
  expect(await storageIndexBytes()).toBe(0)
  await attachEvidence(info)
})
