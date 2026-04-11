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

function resolveDesktopBridgeEnv(env: NodeJS.ProcessEnv) {
  const devIpcPort = Number(env.PATHKEEP_DEV_IPC_PORT ?? '43118')
  return {
    devIpcUrl: `http://127.0.0.1:${devIpcPort}`,
  }
}

test.describe.configure({ mode: 'serial' })

test('connects chrome to the live desktop command bridge', async ({
  page,
  request,
}) => {
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
      { timeout: 120_000, intervals: [500, 1_000, 2_000] },
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

  expect(snapshot.directories.appRoot).toContain('Application Support')
  expect(snapshot.directories.appRoot).not.toContain('~/')
  expect(snapshot.directories.archiveDatabasePath).toContain(
    'history-vault.sqlite',
  )
})
