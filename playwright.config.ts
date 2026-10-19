/**
 * The E2E suite: the real Rust backend (debug build with the dev IPC bridge),
 * the real Vite frontend, and Playwright driving it like a user.
 *
 * Every run starts from synthetic but real browser profiles (SQLite History
 * files) in a fresh temp folder, with the archive, keyring and project root
 * inside it. The fixture is seeded, and its `now` is written to
 * `fixture.json`, so a run can be repeated exactly:
 *
 *   PATHKEEP_E2E_NOW=<now from fixture.json> bun run test:e2e
 *
 * Projects run in order, because they share one backend and one archive:
 * first-run (onboarding) → read (history, insights; nothing changes) →
 * change (background work, new visits, schedule, imports, lock, archive
 * password) → wipe (delete everything) → upgrade (an archive left by 0.3).
 */
import { copyFileSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { defineConfig, devices } from '@playwright/test'
import { resolveDesktopBridgeEnv } from './scripts/pathkeep-dev-desktop-bridge.mjs'
import { writeSyntheticBrowsers } from './scripts/fixtures/synthetic-browsers.mjs'

const artifactsDir = 'artifacts/e2e'
const FIXTURE_SEED = 7

/**
 * Playwright loads this file in the runner and again in each worker. The
 * runner creates the fixture and exports its location; workers reuse it.
 */
function prepareFixture() {
  if (process.env.PATHKEEP_E2E_FIXTURE) return

  const root = mkdtempSync(path.join(os.tmpdir(), 'pathkeep-e2e-'))
  const now = Number(process.env.PATHKEEP_E2E_NOW) || Date.now()
  const browsers = writeSyntheticBrowsers(path.join(root, 'browsers'), {
    now,
    seed: FIXTURE_SEED,
  })
  const dirs = {
    projectRoot: path.join(root, 'project-root'),
    keyring: path.join(root, 'keyring'),
    noSafari: path.join(root, 'no-safari'),
    osSandbox: path.join(root, 'os-sandbox'),
  }
  for (const dir of Object.values(dirs)) mkdirSync(dir, { recursive: true })

  const fixtureFile = path.join(root, 'fixture.json')
  writeFileSync(
    fixtureFile,
    JSON.stringify({ root, now, seed: FIXTURE_SEED, ...browsers, dirs }),
  )
  mkdirSync(artifactsDir, { recursive: true })
  copyFileSync(fixtureFile, path.join(artifactsDir, 'fixture.json'))

  Object.assign(process.env, {
    PATHKEEP_E2E_FIXTURE: fixtureFile,
    CHB_PROJECT_ROOT: dirs.projectRoot,
    CHB_CHROME_USER_DATA_DIR: browsers.chromeUserDataRoot,
    CHB_FIREFOX_PROFILES_DIR: browsers.firefoxProfilesRoot,
    CHB_SAFARI_ROOT: dirs.noSafari,
    CHB_TEST_KEYRING_DIR: dirs.keyring,
    // Schedules and login items go to files here, never to the real
    // launchd / Task Scheduler. Debug builds only; release ignores it.
    PATHKEEP_PLATFORM_TEST_SANDBOX_DIR: dirs.osSandbox,
    PATHKEEP_TEST_BLOCK_NETWORK: '1',
    // Link previews may still reach link-preview.spec.ts's own HTTPS server
    // on 127.0.0.1, which this authority signed. Debug only.
    PATHKEEP_TEST_PREVIEW_SERVER_CA: path.resolve(
      'tests/e2e/support/tls/ca.pem',
    ),
    // Where a save dialog "saves": Playwright cannot answer the system's
    // dialog, so the app saves here under the suggested name. Debug only.
    PATHKEEP_TEST_SAVE_DIR: path.join(root, 'saved'),
    // Lets export.spec.ts watch and stop an export of the small fixture. Debug only.
    PATHKEEP_TEST_EXPORT_ROW_DELAY_US: '500',
    // The launcher runs the app itself and starts it again when a spec kills
    // it after creating this file (semantic-index.spec.ts).
    PATHKEEP_E2E_RESTART_FILE: path.join(root, 'restart-app'),
    // Keep the build cache between runs; a cold Rust build takes minutes.
    CARGO_TARGET_DIR:
      process.env.CARGO_TARGET_DIR ??
      path.resolve('var/playwright/desktop-bridge/cargo-target'),
  })
  if (!process.env.PATHKEEP_DEV_SERVER_PORT) {
    const offset = Math.trunc(Math.random() * 2_000)
    process.env.PATHKEEP_DEV_SERVER_PORT = String(15_420 + offset)
    process.env.PATHKEEP_DEV_IPC_PORT = String(43_118 + offset)
  }
}

function project(name: string, files: string[], after?: string) {
  return {
    name,
    testMatch: files,
    dependencies: after ? [after] : [],
    use: {
      ...devices['Desktop Chrome'],
      viewport: { width: 1440, height: 900 },
    },
  }
}

prepareFixture()
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
  outputDir: `${artifactsDir}/test-results`,
  reporter: [
    process.env.CI ? ['github'] : ['list'],
    ['html', { open: 'never', outputFolder: `${artifactsDir}/report` }],
    ['json', { outputFile: `${artifactsDir}/results.json` }],
    ['./tests/e2e/support/thread-stacks-reporter.ts'],
  ],
  use: {
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
    // Vite is up long before the Rust backend; wait for the backend.
    url: `${bridge.devIpcUrl}/health`,
    reuseExistingServer: false,
    // A cold debug build of the backend takes several minutes.
    timeout: 900_000,
    stdout: 'pipe',
  },
  projects: [
    project('first-run', ['first-run.spec.ts']),
    project(
      'read',
      [
        'history.spec.ts',
        'history-tools.spec.ts',
        'insights.spec.ts',
        'insights-counts.spec.ts',
        'insights-drill.spec.ts',
        'export.spec.ts',
        'search-loading.spec.ts',
        'search-limits.spec.ts',
        'shell-and-lists.spec.ts',
        'links.spec.ts',
        'home.spec.ts',
        'clock.spec.ts',
      ],
      'first-run',
    ),
    // A project runs its files in name order, so what must come first or
    // last gets a project of its own.
    // It counts the fixture's visits, before the specs after it add some.
    project('zone', ['insights-time-zone.spec.ts'], 'read'),
    project(
      'change',
      [
        'ai-services.spec.ts',
        'background-work.spec.ts',
        'backup.spec.ts',
        'link-preview.spec.ts',
        'search-ranking.spec.ts',
        'schedule.spec.ts',
        'browser-import.spec.ts',
        'home-alerts.spec.ts',
        'takeout-import.spec.ts',
        'ask-chats.spec.ts',
        'ask-agent.spec.ts',
        'ask.spec.ts',
        'lock.spec.ts',
        'mcp-log.spec.ts',
        'security.spec.ts',
        'site-category.spec.ts',
      ],
      'zone',
    ),
    // They kill the app and the launcher starts it again, behind App Lock.
    project(
      'restart',
      [
        'insights-reopen.spec.ts',
        'semantic-index.spec.ts',
        'takeout-continue.spec.ts',
      ],
      'change',
    ),
    // Large imports, after the semantic index is built so it does not embed
    // their pages (a project runs its files in name order, so not in restart).
    project('stop', ['import-stop.spec.ts'], 'restart'),
    // Opening the moved file replaces the archive and its password, so it runs
    // after every test that relies on the archive first-run created.
    project('move', ['move.spec.ts'], 'stop'),
    project('wipe', ['wipe.spec.ts'], 'move'),
    // Needs the empty data folder wipe leaves behind.
    project('upgrade', ['upgrade.spec.ts'], 'wipe'),
  ],
})
