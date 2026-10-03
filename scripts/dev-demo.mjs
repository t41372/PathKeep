/**
 * Runs the real desktop app in browser-bridge mode against a synthetic demo
 * archive, for UI work and manual checks without touching your own history.
 *
 *   bun run dev:demo            # reuse var/demo if it exists
 *   bun run dev:demo -- --fresh # wipe var/demo and start over
 *   bun run dev:demo -- --backend path/to/pathkeep-desktop
 *                               # skip cargo: run a prebuilt debug backend
 *                               # (its pathkeep-worker must sit next to it)
 *   bun run dev:demo -- --first-run --fresh
 *                               # leave the archive uninitialized, so the
 *                               # app opens on onboarding (E2E uses this)
 *
 * Steps: write synthetic Chrome/Firefox profiles, start the Tauri backend
 * with the dev IPC bridge (under xvfb on headless Linux), then initialize
 * the archive and run the first backup through the bridge if needed.
 * Open http://127.0.0.1:1420 in a browser when it says ready.
 */
import { spawn, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, rmSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { resolveDesktopBridgeEnv } from './pathkeep-dev-desktop-bridge.mjs'
import { writeSyntheticBrowsers } from './fixtures/synthetic-browsers.mjs'

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
)
const demoRoot = path.join(repoRoot, 'var', 'demo')
const fresh = process.argv.includes('--fresh')
const firstRun = process.argv.includes('--first-run')
const backendFlag = process.argv.indexOf('--backend')
const prebuiltBackend =
  backendFlag >= 0 ? path.resolve(process.argv[backendFlag + 1] ?? '') : null

if (fresh) rmSync(demoRoot, { recursive: true, force: true })
const browsersRoot = path.join(demoRoot, 'browsers')
const projectRoot = path.join(demoRoot, 'project-root')
const keyringRoot = path.join(demoRoot, 'keyring')
const emptySafari = path.join(demoRoot, 'no-safari')
for (const dir of [projectRoot, keyringRoot, emptySafari])
  mkdirSync(dir, { recursive: true })

let browsers
if (!existsSync(path.join(browsersRoot, 'chrome-user-data'))) {
  browsers = writeSyntheticBrowsers(browsersRoot, {})
  console.log(
    `Wrote ${browsers.counts.total} synthetic visits to ${browsersRoot}`,
  )
} else {
  browsers = {
    chromeUserDataRoot: path.join(browsersRoot, 'chrome-user-data'),
    firefoxProfilesRoot: path.join(browsersRoot, 'firefox', 'Profiles'),
  }
}

const bridge = resolveDesktopBridgeEnv()
const env = {
  ...process.env,
  CHB_PROJECT_ROOT: projectRoot,
  CHB_CHROME_USER_DATA_DIR: browsers.chromeUserDataRoot,
  CHB_FIREFOX_PROFILES_DIR: browsers.firefoxProfilesRoot,
  CHB_SAFARI_ROOT: emptySafari,
  // Debug builds keep schedule and login-item state here instead of the
  // real launchd / Task Scheduler / login items.
  PATHKEEP_PLATFORM_TEST_SANDBOX_DIR: path.join(demoRoot, 'os-sandbox'),
  CHB_TEST_KEYRING_DIR: keyringRoot,
  // A label of its own as well, so even a sandbox mix-up could never touch
  // the real PathKeep LaunchAgent.
  PATHKEEP_PLATFORM_TEST_SCHEDULE_LABEL: 'com.yi-ting.pathkeep.demo.backup',
  CARGO_TARGET_DIR:
    process.env.CARGO_TARGET_DIR ??
    path.join(repoRoot, 'var', 'playwright', 'desktop-bridge', 'cargo-target'),
}

const headless = process.platform === 'linux' && !process.env.DISPLAY
const hasXvfb =
  spawnSync('sh', ['-c', 'command -v xvfb-run'], { stdio: 'ignore' }).status ===
  0
const underDisplay = (command, args) =>
  headless && hasXvfb ? ['xvfb-run', ['-a', command, ...args]] : [command, args]

const children = []
function start(command, args, extraEnv = {}) {
  const child = spawn(command, args, {
    cwd: repoRoot,
    env: { ...env, ...extraEnv },
    stdio: 'inherit',
  })
  child.on('exit', (code) => {
    for (const other of children) if (other !== child) other.kill('SIGTERM')
    process.exit(code ?? 0)
  })
  children.push(child)
}

if (prebuiltBackend) {
  // Mirrors what `tauri dev --features devtools-bridge` sets up, minus the build.
  const bridgeEnv = {
    PATHKEEP_ENABLE_DEV_IPC_BRIDGE: '1',
    PATHKEEP_DEV_SERVER_PORT: String(bridge.devServerPort),
    PATHKEEP_DEV_IPC_PORT: String(bridge.devIpcPort),
    PATHKEEP_DEV_IPC_ALLOWED_ORIGINS: `${bridge.devServerUrl},http://localhost:${bridge.devServerPort}`,
    VITE_PATHKEEP_DEV_IPC_URL: bridge.devIpcUrl,
  }
  start('bun', ['run', 'dev'], bridgeEnv)
  start(...underDisplay(prebuiltBackend, []), bridgeEnv)
} else {
  start(...underDisplay('bun', ['run', 'desktop:dev:bridge']))
}
for (const signal of ['SIGINT', 'SIGTERM'])
  process.on(signal, () => {
    for (const child of children) child.kill(signal)
    process.exit(0)
  })

async function invoke(name, payload = {}) {
  const response = await fetch(`${bridge.devIpcUrl}/commands/${name}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  })
  const body = await response.json()
  if (!response.ok) throw new Error(`${name}: ${JSON.stringify(body)}`)
  return body
}

async function waitForBridge() {
  for (let attempt = 0; attempt < 900; attempt += 1) {
    try {
      const health = await fetch(`${bridge.devIpcUrl}/health`)
      const page = await fetch(bridge.devServerUrl)
      if (health.ok && page.ok) return
    } catch {
      // Still compiling or starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 2_000))
  }
  throw new Error('The desktop bridge did not come up within 30 minutes.')
}

await waitForBridge()
const snapshot = await invoke('app_snapshot')
if (firstRun) {
  if (snapshot.config.initialized) {
    console.warn(
      'The demo archive is already set up. Add --fresh to see onboarding.',
    )
  }
} else if (!snapshot.config.initialized) {
  const selectedProfileIds = snapshot.browserProfiles
    // The Firefox override also matches Floorp, LibreWolf etc.; keep one copy.
    .filter(
      (profile) =>
        profile.historyExists && /^(chrome|firefox):/.test(profile.profileId),
    )
    .map((profile) => profile.profileId)
  await invoke('initialize_archive', {
    config: { ...snapshot.config, gitEnabled: false, selectedProfileIds },
  })
  const report = await invoke('run_backup_now', { dueOnly: false })
  console.log(
    `Demo archive ready: ${report.run?.newVisits ?? 0} visits from ${selectedProfileIds.length} profiles.`,
  )
}
console.log(`PathKeep demo is ready at ${bridge.devServerUrl}`)
