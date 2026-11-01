/** Runs the backup repair contract against an actual v1 archive and real synthetic browsers. */
import { execFileSync } from 'node:child_process'
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { defineConfig, devices } from '@playwright/test'
import {
  e2eCargoTargetDir,
  resolveDesktopBridgeEnv,
} from './scripts/pathkeep-dev-desktop-bridge.mjs'

const variant =
  process.env.PATHKEEP_BACKUP_INTEGRITY_VARIANT === 'damaged'
    ? 'damaged'
    : 'healthy'
const artifacts = `artifacts/e2e/backup-integrity-${variant}`
if (!process.env.PATHKEEP_BACKUP_INTEGRITY_FIXTURE) {
  // `run-playwright.mjs` makes the folder and deletes it after a run that passed.
  const root =
    process.env.PATHKEEP_E2E_ROOT ??
    mkdtempSync(path.join(os.tmpdir(), 'pathkeep-rescue-e2e-'))
  execFileSync('tar', [
    '-xzf',
    path.resolve(`tests/fixtures/native-v1/${variant}.tar.gz`),
    '-C',
    root,
  ])
  const dirs = {
    projectRoot: path.join(root, 'project-root'),
    keyring: path.join(root, 'keyring'),
    noSafari: path.join(root, 'no-safari'),
    osSandbox: path.join(root, 'os-sandbox'),
  }
  for (const dir of Object.values(dirs)) mkdirSync(dir, { recursive: true })
  const files = {
    'chrome:Default': path.join(root, 'browsers/chrome/Default/History'),
    'chrome:Profile 1': path.join(root, 'browsers/chrome/Profile 1/History'),
    'firefox:k3x9.default-release': path.join(
      root,
      'browsers/firefox/Profiles/k3x9.default-release/places.sqlite',
    ),
  }
  const file = path.join(root, 'fixture.json')
  const before = JSON.parse(
    readFileSync(
      path.join(dirs.projectRoot, 'expected-before-upgrade.json'),
      'utf8',
    ),
  )
  writeFileSync(
    file,
    JSON.stringify({
      root,
      dirs,
      files,
      now: 1793188800000,
      seed: 0,
      visitsByProfile: {},
      before,
    }),
  )
  Object.assign(process.env, {
    PATHKEEP_BACKUP_INTEGRITY_FIXTURE: file,
    PATHKEEP_E2E_FIXTURE: file,
    CHB_PROJECT_ROOT: dirs.projectRoot,
    CHB_CHROME_USER_DATA_DIR: path.join(root, 'browsers/chrome'),
    CHB_FIREFOX_PROFILES_DIR: path.join(root, 'browsers/firefox/Profiles'),
    CHB_SAFARI_ROOT: dirs.noSafari,
    CHB_TEST_KEYRING_DIR: dirs.keyring,
    PATHKEEP_PLATFORM_TEST_SANDBOX_DIR: dirs.osSandbox,
    PATHKEEP_TEST_BLOCK_NETWORK: '1',
    PATHKEEP_TEST_SAVE_DIR: path.join(root, 'saved'),
    PATHKEEP_E2E_RESTART_FILE: path.join(root, 'restart-app'),
    CARGO_TARGET_DIR: e2eCargoTargetDir(),
  })
  mkdirSync(artifacts, { recursive: true })
  copyFileSync(file, path.join(artifacts, 'fixture.json'))
}
if (!process.env.PATHKEEP_DEV_SERVER_PORT) {
  const offset = Math.trunc(Math.random() * 2000)
  process.env.PATHKEEP_DEV_SERVER_PORT = String(15420 + offset)
  process.env.PATHKEEP_DEV_IPC_PORT = String(43118 + offset)
}
const bridge = resolveDesktopBridgeEnv(process.env)
const headlessLinux = process.platform === 'linux' && !process.env.DISPLAY
export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  workers: 1,
  forbidOnly: Boolean(process.env.CI),
  retries: 0,
  timeout: 120_000,
  expect: { timeout: 15_000 },
  use: {
    ...devices['Desktop Chrome'],
    baseURL: bridge.devServerUrl,
    viewport: { width: 1440, height: 900 },
    locale: 'en-US',
    trace: 'on',
    screenshot: 'on',
    video: 'off',
    launchOptions: {
      executablePath:
        process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH || undefined,
    },
  },
  webServer: {
    command: `${headlessLinux ? 'xvfb-run -a ' : ''}bun run desktop:dev:bridge`,
    url: `${bridge.devIpcUrl}/health`,
    reuseExistingServer: false,
    timeout: 900_000,
    stdout: 'pipe',
  },
  outputDir: `${artifacts}/test-results`,
  reporter: [
    ['list'],
    ['html', { open: 'never', outputFolder: `${artifacts}/report` }],
    ['json', { outputFile: `${artifacts}/results.json` }],
    ['./tests/e2e/support/thread-stacks-reporter.ts'],
  ],
  projects: [
    {
      name: 'backup-integrity',
      testMatch:
        variant === 'healthy'
          ? 'backup-integrity.spec.ts'
          : 'backup-integrity-damaged.spec.ts',
    },
  ],
})
