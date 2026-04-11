import { defineConfig, devices } from '@playwright/test'
import { resolveDesktopBridgeEnv } from './scripts/pathkeep-dev-desktop-bridge.mjs'

const desktopBridgeEnv = resolveDesktopBridgeEnv({
  ...process.env,
  PATHKEEP_DEV_SERVER_PORT: process.env.PATHKEEP_DEV_SERVER_PORT ?? '15420',
  PATHKEEP_DEV_IPC_PORT: process.env.PATHKEEP_DEV_IPC_PORT ?? '43118',
})

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: 'desktop-bridge.spec.ts',
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI
    ? [['github'], ['html', { open: 'never' }]]
    : [['list']],
  use: {
    baseURL: desktopBridgeEnv.devServerUrl,
    trace: 'retain-on-failure',
  },
  webServer: {
    command: `PATHKEEP_DEV_SERVER_PORT=${desktopBridgeEnv.devServerPort} PATHKEEP_DEV_IPC_PORT=${desktopBridgeEnv.devIpcPort} bun run desktop:dev:bridge`,
    url: desktopBridgeEnv.devServerUrl,
    reuseExistingServer: false,
    timeout: 180_000,
  },
  projects: [
    {
      name: 'chrome-desktop-bridge',
      use: {
        ...devices['Desktop Chrome'],
        channel: 'chrome',
      },
    },
  ],
})
