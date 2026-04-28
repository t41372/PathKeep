import { defineConfig, devices } from '@playwright/test'

const macosSandboxLaunchArgs =
  process.platform === 'darwin' ? ['--single-process'] : []

export default defineConfig({
  testDir: './tests/e2e',
  testIgnore: 'desktop-bridge.spec.ts',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI
    ? [['github'], ['html', { open: 'never' }]]
    : [['list']],
  use: {
    baseURL: 'http://127.0.0.1:1420',
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'bun run dev',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        launchOptions: {
          args: macosSandboxLaunchArgs,
        },
      },
    },
  ],
})
