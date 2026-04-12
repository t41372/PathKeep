import { DatabaseSync } from 'node:sqlite'
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { defineConfig, devices } from '@playwright/test'
import { resolveDesktopBridgeEnv } from './scripts/pathkeep-dev-desktop-bridge.mjs'

function chromeTimestampMicros(timestampMs: number) {
  return Math.trunc(timestampMs * 1_000 + 11_644_473_600_000_000)
}

function prepareDesktopBridgeFixture() {
  // Use a synthetic Chrome profile so the desktop bridge can exercise the real Rust archive path in CI.
  const fixtureRoot = mkdtempSync(
    path.join(os.tmpdir(), 'pathkeep-desktop-bridge-'),
  )
  const projectRoot = path.join(fixtureRoot, 'project-root')
  const chromeUserDataRoot = path.join(fixtureRoot, 'chrome-user-data')
  const profileRoot = path.join(chromeUserDataRoot, 'Default')
  const keyringRoot = path.join(fixtureRoot, 'keyring')
  const cargoTargetDir = path.join(fixtureRoot, 'cargo-target')

  mkdirSync(projectRoot, { recursive: true })
  mkdirSync(chromeUserDataRoot, { recursive: true })
  mkdirSync(profileRoot, { recursive: true })
  mkdirSync(keyringRoot, { recursive: true })
  mkdirSync(cargoTargetDir, { recursive: true })

  writeFileSync(path.join(chromeUserDataRoot, 'Last Version'), '135.0.0.0')
  writeFileSync(
    path.join(chromeUserDataRoot, 'Local State'),
    JSON.stringify({
      profile: {
        info_cache: {
          Default: {
            name: 'Default',
            user_name: 'fixture@example.test',
          },
        },
      },
    }),
  )

  const history = new DatabaseSync(path.join(profileRoot, 'History'))
  history.exec(`
    CREATE TABLE urls (
      id INTEGER PRIMARY KEY,
      url TEXT NOT NULL,
      title TEXT,
      visit_count INTEGER NOT NULL,
      typed_count INTEGER NOT NULL,
      last_visit_time INTEGER NOT NULL,
      hidden INTEGER NOT NULL
    );
    CREATE TABLE visits (
      id INTEGER PRIMARY KEY,
      url INTEGER NOT NULL,
      visit_time INTEGER NOT NULL,
      from_visit INTEGER,
      transition INTEGER,
      visit_duration INTEGER,
      is_known_to_sync INTEGER,
      visited_link_id INTEGER,
      external_referrer_url TEXT,
      app_id TEXT
    );
    CREATE TABLE downloads (
      id INTEGER PRIMARY KEY,
      guid TEXT,
      current_path TEXT,
      target_path TEXT,
      start_time INTEGER,
      received_bytes INTEGER,
      total_bytes INTEGER,
      state INTEGER,
      mime_type TEXT,
      original_mime_type TEXT
    );
    CREATE TABLE keyword_search_terms (
      keyword_id INTEGER,
      url_id INTEGER,
      term TEXT,
      normalized_term TEXT
    );
  `)

  const now = chromeTimestampMicros(Date.now())
  const yesterday = chromeTimestampMicros(Date.now() - 86_400_000)
  const insertUrl = history.prepare(
    'INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden) VALUES (?, ?, ?, ?, ?, ?, ?)',
  )
  const insertVisit = history.prepare(
    'INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)',
  )
  const insertDownload = history.prepare(
    'INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)',
  )
  const insertSearchTerm = history.prepare(
    'INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term) VALUES (?, ?, ?, ?)',
  )

  insertUrl.run(
    1,
    'https://www.google.com/search?q=example',
    'example - Google Search',
    1,
    1,
    now,
    0,
  )
  insertVisit.run(
    1,
    1,
    now,
    null,
    805306368,
    24_000,
    1,
    3,
    'https://ref.example',
    'com.example.app',
  )
  insertSearchTerm.run(1, 1, 'example', 'example')

  insertUrl.run(
    2,
    'https://pathkeep.example.com/insights',
    'PathKeep insights',
    1,
    0,
    yesterday,
    0,
  )
  insertVisit.run(2, 2, yesterday, null, 805306368, 12_000, 1, 7, null, null)
  insertSearchTerm.run(2, 2, 'pathkeep insights', 'pathkeep insights')

  insertUrl.run(
    3,
    'https://docs.example.com/pathkeep/bridge',
    'PathKeep desktop bridge',
    1,
    0,
    now - 3_600_000,
    0,
  )
  insertVisit.run(
    3,
    3,
    now - 3_600_000,
    null,
    805306368,
    18_000,
    1,
    11,
    null,
    null,
  )

  insertDownload.run(
    1,
    'guid-1',
    '/tmp/current',
    '/tmp/target',
    1,
    1,
    2,
    3,
    'text/html',
    'text/plain',
  )

  history.close()

  process.env.CHB_PROJECT_ROOT = projectRoot
  process.env.CHB_CHROME_USER_DATA_DIR = chromeUserDataRoot
  process.env.CHB_TEST_KEYRING_DIR = keyringRoot
  process.env.CARGO_TARGET_DIR = cargoTargetDir

  return {
    projectRoot,
    chromeUserDataRoot,
    keyringRoot,
    cargoTargetDir,
    profileId: 'chrome:Default',
  }
}

prepareDesktopBridgeFixture()
const desktopBridgeEnv = resolveDesktopBridgeEnv({
  ...process.env,
  PATHKEEP_DEV_SERVER_PORT: process.env.PATHKEEP_DEV_SERVER_PORT ?? '15420',
  PATHKEEP_DEV_IPC_PORT: process.env.PATHKEEP_DEV_IPC_PORT ?? '43118',
})

const desktopBridgeCommand =
  process.platform === 'linux' && process.env.CI
    ? `PATHKEEP_DEV_SERVER_PORT=${desktopBridgeEnv.devServerPort} PATHKEEP_DEV_IPC_PORT=${desktopBridgeEnv.devIpcPort} xvfb-run -a bun run desktop:dev:bridge`
    : `PATHKEEP_DEV_SERVER_PORT=${desktopBridgeEnv.devServerPort} PATHKEEP_DEV_IPC_PORT=${desktopBridgeEnv.devIpcPort} bun run desktop:dev:bridge`

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
    command: desktopBridgeCommand,
    url: desktopBridgeEnv.devServerUrl,
    reuseExistingServer: false,
    timeout: 180_000,
  },
  projects: [
    {
      name: 'chrome-desktop-bridge',
      use: {
        ...devices['Desktop Chrome'],
      },
    },
  ],
})
