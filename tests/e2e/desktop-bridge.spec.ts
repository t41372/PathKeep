import { expect, test } from '@playwright/test'

interface BridgeHealth {
  runtime?: string
}

interface BridgeBuildInfo {
  productName: string
  gitCommitShort: string
  version: string
}

interface BridgeSnapshot {
  directories: {
    appRoot: string
    archiveDatabasePath: string
  }
}

interface BridgeAppSnapshot extends BridgeSnapshot {
  archiveStatus: {
    initialized: boolean
  }
  browserProfiles: Array<{
    profileId: string
  }>
}

interface BridgeBackupReport {
  dueSkipped: boolean
  manifestPath?: string | null
  run?: {
    id: number
    newVisits: number
  }
}

interface BridgeDashboardSnapshot {
  totalVisits: number
  recentRuns: Array<{
    id: number
    newVisits: number
  }>
  lastSuccessfulBackupAt?: string | null
}

interface BridgeHistoryResponse {
  total: number
  items: Array<{
    url: string
  }>
}

interface BridgeCoreIntelligenceRebuildReport {
  processedVisits: number
  sessions: number
  searchTrails: number
  lastRunAt: string
  notes: string[]
}

interface BridgeSectionResult<T> {
  data: T
}

interface BridgeDigestSummary {
  totalVisits: {
    value: number
  }
  totalSearches: {
    value: number
  }
}

interface BridgeSessionListResult {
  total: number
  sessions: Array<{
    sessionId: string
  }>
}

interface BridgePublicSnapshot {
  topDomains: string[]
  notes: string[]
}

function resolveDesktopBridgeEnv(env: NodeJS.ProcessEnv) {
  const devIpcPort = Number(env.PATHKEEP_DEV_IPC_PORT ?? '43118')
  return {
    devIpcUrl: `http://127.0.0.1:${devIpcPort}`,
  }
}

async function invokeDesktopBridge<T>(
  request: {
    post: (
      url: string,
      options?: { data?: unknown },
    ) => Promise<{
      ok(): boolean
      status(): number
      text(): Promise<string>
      json(): Promise<unknown>
    }>
  },
  bridgeUrl: string,
  command: string,
  payload: unknown = {},
): Promise<T> {
  const response = await request.post(`${bridgeUrl}/commands/${command}`, {
    data: payload,
  })

  if (!response.ok()) {
    throw new Error(
      `Bridge command ${command} failed with ${response.status()}: ${await response.text()}`,
    )
  }

  return (await response.json()) as T
}

test.describe.configure({ mode: 'parallel' })

const bridgeReadyTimeoutMs = 300_000

test('treats unreachable bridge health checks as not-ready', async ({
  request,
}, testInfo) => {
  testInfo.setTimeout(600_000)

  const status = await (async () => {
    try {
      const health = await request.get('http://127.0.0.1:9/health', {
        timeout: 1_000,
      })
      if (!health.ok()) {
        return 'not-ready'
      }
      const payload = (await health.json()) as BridgeHealth
      return payload.runtime ?? 'not-ready'
    } catch {
      return 'not-ready'
    }
  })()

  expect(status).toBe('not-ready')
})

test('connects chrome to the live desktop command bridge', async ({
  page,
  request,
}, testInfo) => {
  testInfo.setTimeout(600_000)

  const { devIpcUrl } = resolveDesktopBridgeEnv({
    ...process.env,
    PATHKEEP_DEV_SERVER_PORT: process.env.PATHKEEP_DEV_SERVER_PORT ?? '15420',
    PATHKEEP_DEV_IPC_PORT: process.env.PATHKEEP_DEV_IPC_PORT ?? '43118',
  })
  await expect
    .poll(
      async () => {
        try {
          const health = await request.get(`${devIpcUrl}/health`)
          if (!health.ok()) {
            return 'not-ready'
          }
          const payload = (await health.json()) as BridgeHealth
          return payload.runtime ?? 'not-ready'
        } catch {
          return 'not-ready'
        }
      },
      { timeout: bridgeReadyTimeoutMs, intervals: [500, 1_000, 2_000, 5_000] },
    )
    .toBe('browser-desktop-bridge')

  await page.goto('/')
  await expect(page.locator('html')).toHaveAttribute(
    'data-pathkeep-runtime',
    'browser-desktop-bridge',
  )

  const buildInfo = await page.evaluate(
    async (bridgeUrl): Promise<BridgeBuildInfo> => {
      const response = await fetch(`${bridgeUrl}/commands/app_build_info`, {
        method: 'POST',
      })

      return (await response.json()) as BridgeBuildInfo
    },
    devIpcUrl,
  )

  expect(buildInfo.productName).toBe('PathKeep')
  expect(buildInfo.version).toBeTruthy()
  expect(buildInfo.gitCommitShort).not.toBe('preview')

  const snapshot = await page.evaluate(
    async (bridgeUrl): Promise<BridgeSnapshot> => {
      const response = await fetch(`${bridgeUrl}/commands/app_snapshot`, {
        method: 'POST',
      })

      return (await response.json()) as BridgeSnapshot
    },
    devIpcUrl,
  )

  const expectedAppRoot = process.env.CHB_PROJECT_ROOT
  if (expectedAppRoot) {
    expect(snapshot.directories.appRoot).toBe(expectedAppRoot)
  } else {
    expect(snapshot.directories.appRoot).toContain('Application Support')
  }
  expect(snapshot.directories.appRoot).not.toContain('~/')
  expect(snapshot.directories.archiveDatabasePath).toContain(
    'history-vault.sqlite',
  )
})

test('runs a live backup and core intelligence flow through the desktop command bridge', async ({
  page,
  request,
}, testInfo) => {
  testInfo.setTimeout(600_000)

  const { devIpcUrl } = resolveDesktopBridgeEnv({
    ...process.env,
    PATHKEEP_DEV_SERVER_PORT: process.env.PATHKEEP_DEV_SERVER_PORT ?? '15420',
    PATHKEEP_DEV_IPC_PORT: process.env.PATHKEEP_DEV_IPC_PORT ?? '43118',
  })

  await expect
    .poll(
      async () => {
        try {
          const health = await request.get(`${devIpcUrl}/health`)
          if (!health.ok()) {
            return 'not-ready'
          }
          const payload = (await health.json()) as BridgeHealth
          return payload.runtime ?? 'not-ready'
        } catch {
          return 'not-ready'
        }
      },
      { timeout: bridgeReadyTimeoutMs, intervals: [500, 1_000, 2_000, 5_000] },
    )
    .toBe('browser-desktop-bridge')

  const appBuildInfo = await invokeDesktopBridge<BridgeBuildInfo>(
    request,
    devIpcUrl,
    'app_build_info',
  )
  expect(appBuildInfo.productName).toBe('PathKeep')
  expect(appBuildInfo.version).toBeTruthy()

  const initialized = await invokeDesktopBridge<BridgeAppSnapshot>(
    request,
    devIpcUrl,
    'initialize_archive',
    {
      config: {
        gitEnabled: false,
        selectedProfileIds: ['chrome:Default'],
        ai: {
          enabled: true,
          assistantEnabled: true,
          semanticIndexEnabled: true,
          mcpEnabled: true,
          skillEnabled: true,
          llmProviderId: 'llm-primary',
          embeddingProviderId: 'embed-primary',
          llmProviders: [
            {
              id: 'llm-primary',
              name: 'Primary LLM',
              purpose: 'llm',
              requestFormat: 'openai',
              enabled: true,
              defaultModel: 'gpt-4.1-mini',
            },
          ],
          embeddingProviders: [
            {
              id: 'embed-primary',
              name: 'Primary embedding',
              purpose: 'embedding',
              requestFormat: 'openai',
              enabled: true,
              defaultModel: 'text-embedding-3-large',
              dimensions: 1536,
            },
          ],
        },
      },
      databaseKey: null,
    },
  )

  expect(initialized.archiveStatus.initialized).toBe(true)
  expect(initialized.browserProfiles).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ profileId: 'chrome:Default' }),
    ]),
  )
  const expectedAppRoot = process.env.CHB_PROJECT_ROOT
  if (expectedAppRoot) {
    expect(initialized.directories.appRoot).toBe(expectedAppRoot)
  } else {
    expect(initialized.directories.appRoot).toContain(
      'pathkeep-desktop-bridge-',
    )
  }

  const backup = await invokeDesktopBridge<BridgeBackupReport>(
    request,
    devIpcUrl,
    'run_backup_now',
    {
      dueOnly: false,
    },
  )

  expect(backup.dueSkipped).toBe(false)
  expect(backup.manifestPath).toBeTruthy()
  expect(backup.run?.newVisits).toBeGreaterThan(0)

  const dashboard = await invokeDesktopBridge<BridgeDashboardSnapshot>(
    request,
    devIpcUrl,
    'load_dashboard_snapshot',
  )
  expect(dashboard.totalVisits).toBe(backup.run?.newVisits)
  expect(dashboard.recentRuns[0].id).toBe(backup.run?.id)
  expect(dashboard.recentRuns[0].newVisits).toBe(backup.run?.newVisits)
  expect(dashboard.lastSuccessfulBackupAt).toBeTruthy()

  const history = await invokeDesktopBridge<BridgeHistoryResponse>(
    request,
    devIpcUrl,
    'query_history',
    {
      query: {
        q: 'example',
        limit: 10,
      },
    },
  )
  expect(history.total).toBeGreaterThan(0)
  expect(history.items[0].url).toContain('example')

  const dateRange = {
    start: '2000-01-01',
    end: '2100-01-01',
  }

  const intelligenceRun =
    await invokeDesktopBridge<BridgeCoreIntelligenceRebuildReport>(
      request,
      devIpcUrl,
      'run_core_intelligence_now',
      {
        request: {
          profileId: 'chrome:Default',
          fullRebuild: true,
        },
      },
    )

  expect(intelligenceRun.processedVisits).toBeGreaterThan(0)
  expect(intelligenceRun.sessions).toBeGreaterThan(0)
  expect(intelligenceRun.searchTrails).toBeGreaterThan(0)
  expect(intelligenceRun.lastRunAt).toBeTruthy()
  expect(
    intelligenceRun.notes.some((note) =>
      note.toLowerCase().includes('core intelligence'),
    ),
  ).toBe(true)

  const digest = await invokeDesktopBridge<
    BridgeSectionResult<BridgeDigestSummary>
  >(request, devIpcUrl, 'get_digest_summary', {
    request: {
      dateRange,
      profileId: 'chrome:Default',
    },
  })
  expect(digest.data.totalVisits.value).toBe(backup.run?.newVisits)
  expect(digest.data.totalSearches.value).toBeGreaterThan(0)

  const sessions = await invokeDesktopBridge<BridgeSessionListResult>(
    request,
    devIpcUrl,
    'get_sessions',
    {
      request: {
        dateRange,
        profileId: 'chrome:Default',
        page: 0,
        pageSize: 20,
      },
    },
  )
  expect(sessions.total).toBeGreaterThan(0)
  expect(sessions.sessions[0].sessionId).toBeTruthy()

  const publicSnapshot = await invokeDesktopBridge<BridgePublicSnapshot>(
    request,
    devIpcUrl,
    'get_intelligence_public_snapshot',
    {
      request: {
        dateRange,
        profileId: 'chrome:Default',
      },
    },
  )
  expect(publicSnapshot.topDomains.length).toBeGreaterThan(0)
  expect(
    publicSnapshot.notes.some((note) =>
      note.includes('omit visit-level identifiers'),
    ),
  ).toBe(true)

  await page.goto('/')
  await expect(page.locator('html')).toHaveAttribute(
    'data-pathkeep-runtime',
    'browser-desktop-bridge',
  )
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
})
