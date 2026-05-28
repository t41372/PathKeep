import { defineConfig, devices } from '@playwright/test'

const macosSandboxLaunchArgs =
  process.platform === 'darwin' ? ['--single-process'] : []

// On Linux 26.04 the Playwright-managed chrome-headless-shell binary is not
// available (Playwright supportedOSes table lags upstream Ubuntu releases).
// `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` lets the dev box point at a
// system-installed Chrome (`/usr/bin/google-chrome`) so e2e can still run
// without waiting for upstream to bless the OS version.
const linuxExecutableOverride =
  process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ||
  process.env.PLAYWRIGHT_CHROME_EXECUTABLE_PATH ||
  undefined

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
          ...(linuxExecutableOverride
            ? { executablePath: linuxExecutableOverride }
            : {}),
        },
      },
    },
  ],
})
