/**
 * Makes a PathKeep v0.3.0 data folder with v0.3.0's own app, for the upgrade
 * test of later versions. Runs only in a v0.3.0 checkout, through the dev
 * bridge, with the browser data from playwright.v03-archive.config.ts.
 *
 * Every step is v0.3.0 code: an encrypted archive, a backup of two Chrome
 * profiles, a browser-file import, stars, a note, tags, page text fetched from
 * public pages, a saved chat, an Insights build, App Lock, the password in the
 * (test) keychain, and a v0.3.0 bundle of it all. Each reply is saved under
 * replies/ so the upgrade test's expectations come from what v0.3.0 reported.
 *
 * The data folder itself is copied by the caller after the app has exited.
 */
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type APIRequestContext } from '@playwright/test'

export const PASSWORD = 'quokka tide tables 2025'
const bridge = `http://127.0.0.1:${process.env.PATHKEEP_DEV_IPC_PORT}`
const out = path.resolve('artifacts/e2e/v03-archive')
const replies = path.join(out, 'replies')

async function call<T>(
  request: APIRequestContext,
  command: string,
  payload: unknown = {},
  name = command,
): Promise<T> {
  const response = await request.post(`${bridge}/commands/${command}`, {
    data: payload,
    timeout: 900_000,
  })
  const text = await response.text()
  writeFileSync(
    path.join(replies, `${name}.json`),
    JSON.stringify(
      { command, payload, status: response.status(), reply: safeJson(text) },
      null,
      2,
    ),
  )
  if (!response.ok())
    throw new Error(`${command}: ${response.status()} ${text}`)
  return JSON.parse(text) as T
}

function safeJson(text: string) {
  try {
    return JSON.parse(text)
  } catch {
    return text
  }
}

interface HistoryItem {
  id: number
  url: string
  title?: string | null
  profileId?: string
}

async function findVisit(request: APIRequestContext, q: string, url: string) {
  const page = await call<{ items: HistoryItem[] }>(
    request,
    'query_history',
    { query: { q, limit: 50 } },
    `query_history-${q.replace(/\W+/g, '_')}`,
  )
  const item = page.items.find((row) => row.url === url)
  expect(item, `${url} in History`).toBeTruthy()
  return item!
}

test('v0.3.0 makes a data folder', async ({ request }) => {
  rmSync(out, { recursive: true, force: true })
  mkdirSync(replies, { recursive: true })
  await expect
    .poll(
      async () => {
        try {
          return (await request.get(`${bridge}/health`)).ok()
        } catch {
          return false
        }
      },
      { timeout: 1_800_000, intervals: [2_000, 5_000] },
    )
    .toBe(true)

  const info = await call<{ version: string }>(request, 'app_build_info')
  expect(info.version).toBe('0.3.0')
  const snapshot = await call<{ config: Record<string, unknown> }>(
    request,
    'app_snapshot',
    {},
    'app_snapshot-before',
  )
  await call(request, 'initialize_archive', {
    config: {
      ...snapshot.config,
      gitEnabled: false,
      archiveMode: 'Encrypted',
      rememberDatabaseKeyInKeyring: true,
      selectedProfileIds: ['chrome:Default', 'chrome:Profile 1'],
    },
    databaseKey: PASSWORD,
  })
  await call(request, 'keyring_store_database_key', { value: PASSWORD })
  // Turning page fetching on queues a working set of recent pages; with the
  // archive still empty that set is empty, and the synthetic sites (which
  // don't resolve) never take the single fetch worker's time.
  const settings = await call<Record<string, unknown>>(
    request,
    'get_content_fetch_settings',
  )
  await call(request, 'set_content_fetch_settings', {
    settings: { ...settings, enabled: true },
  })
  await call(request, 'run_backup_now', { dueOnly: false })
  await call(request, 'import_browser_history', {
    request: {
      sourcePath: process.env.V03_IMPORT_FILE,
      dryRun: false,
      browserFamily: 'chrome',
      profileId: null,
      browserName: 'Chrome',
      profileName: 'Old laptop',
    },
  })

  // Stars, a note and tags, the way the History screen saves them.
  const quokka = 'https://example.org/%7Equokka?utm_source=old'
  await call(
    request,
    'set_star',
    { request: { entityKind: 'url', entityKey: quokka } },
    'set_star-quokka',
  )
  await call(
    request,
    'set_star',
    { request: { entityKind: 'domain', entityKey: 'tides.example.net' } },
    'set_star-tides',
  )
  await call(request, 'set_url_notes', {
    request: { url: quokka, notes: 'Bring the red notebook' },
  })
  await call(request, 'replace_url_tags', {
    request: {
      url: 'https://tides.example.net/coast',
      tags: ['coast', 'trip'],
    },
  })

  // Page text: v0.3.0 fetches public https pages itself. Fetching was turned
  // on while the archive was empty, so only these two pages are queued.
  for (const [q, url] of [
    ['Example Domain', 'https://example.com/'],
    ['Example Domains', 'https://www.iana.org/help/example-domains'],
  ]) {
    const visit = await findVisit(request, q, url)
    await call(
      request,
      'content_fetch_now',
      {
        request: {
          historyId: visit.id,
          profileId: visit.profileId ?? 'chrome:Default',
          url,
          title: visit.title ?? null,
        },
      },
      `content_fetch_now-${visit.id}`,
    )
    await expect
      .poll(
        async () => {
          await call(
            request,
            'run_ai_queue_jobs',
            { maxJobs: 8 },
            'run_ai_queue_jobs',
          )
          const rows = await call<unknown[]>(
            request,
            'list_visit_enrichment',
            { historyId: visit.id },
            `list_visit_enrichment-${visit.id}`,
          )
          return JSON.stringify(rows).includes('Example')
        },
        { timeout: 300_000, intervals: [2_000, 5_000] },
      )
      .toBe(true)
  }

  // A chat, saved the way Ask saves one.
  await call(request, 'save_ai_conversation', {
    request: {
      id: 'v03-coast-trip',
      title: 'Coast trip questions',
      providerId: null,
      messages: [
        {
          id: 'm1',
          role: 'user',
          content: 'When did I last read about tide tables?',
          reasoning: null,
          toolCallsJson: null,
          status: null,
        },
        {
          id: 'm2',
          role: 'assistant',
          content:
            'You opened the Tide tables page on your Work profile a week ago.',
          reasoning: null,
          toolCallsJson: null,
          status: 'done',
        },
      ],
    },
  })

  await call(request, 'run_core_intelligence_now', {
    request: { profileId: null, fullRebuild: true, limit: null },
  })

  // A v0.3.0 bundle of the same data, for the format-1 import test.
  await call(request, 'export_app_data', {
    targetPath: path.join(out, 'v03-bundle.zip'),
  })

  // Read back what v0.3.0 now reports; the upgrade test compares against it.
  await call(request, 'load_dashboard_snapshot')
  await call(request, 'list_stars', {})
  await call(request, 'list_url_annotations', { limit: 50 })
  await call(request, 'list_ai_conversations', { request: { limit: 20 } })
  await call(request, 'app_snapshot', {}, 'app_snapshot-after')
  for (const q of ['quokka', 'tide', '潮汐', 'Example'])
    await call(
      request,
      'query_history',
      { query: { q, limit: 50 } },
      `query_history-final-${q}`,
    )

  // App Lock last: from here on the archive needs the passcode first.
  await call(request, 'set_app_lock_passcode', {
    request: { passcode: '4826', recoveryHint: 'the tide' },
  })

  writeFileSync(
    path.join(out, 'made.json'),
    JSON.stringify(
      {
        version: info.version,
        password: PASSWORD,
        appLockPasscode: '4826',
        seeded: JSON.parse(process.env.V03_SEEDED ?? '{}'),
        projectRoot: process.env.CHB_PROJECT_ROOT,
      },
      null,
      2,
    ),
  )
})
