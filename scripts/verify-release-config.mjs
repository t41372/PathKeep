/**
 * Verifies release configuration that must not drift silently.
 *
 * ## Responsibilities
 * - Keep release URLs pointed at the PathKeep repository.
 * - Keep the Windows release path unsigned and buildable without Windows code-signing secrets.
 * - Keep the Windows installer self-contained enough to install WebView2 from the bundle.
 *
 * ## Not responsible for
 * - Building installers or proving a specific Windows host can launch them.
 * - Checking macOS notarization, updater metadata signing, or GitHub secret values.
 * - Replacing the full `bun run check` release gate.
 *
 * ## Dependencies
 * - Node.js built-ins only.
 *
 * ## Performance notes
 * - Reads a small fixed set of configuration files and performs string/JSON checks.
 */
import { readFileSync } from 'node:fs'

const readText = (path) => readFileSync(path, 'utf8')
const readJson = (path) => JSON.parse(readText(path))

const failures = []

function assert(condition, message) {
  if (!condition) failures.push(message)
}

function includes(text, needle, label) {
  assert(text.includes(needle), `${label} must include ${needle}`)
}

function excludes(text, needle, label) {
  assert(!text.includes(needle), `${label} must not include ${needle}`)
}

const tauriConfig = readJson('src-tauri/tauri.conf.json')
const releaseWorkflow = readText('.github/workflows/release.yml')
const updaterSource = readText('src-tauri/src/updater.rs')
const frontendUpdaterSource = readText('src/lib/update.ts')
const previewShellCommands = readText(
  'src/lib/backend-preview-shell-commands.ts',
)
const issueTemplate = readText('.github/ISSUE_TEMPLATE/config.yml')

assert(
  tauriConfig.plugins?.updater?.endpoints?.includes(
    'https://github.com/t41372/PathKeep/releases/latest/download/latest.json',
  ),
  'Tauri updater endpoint must point at t41372/PathKeep',
)
assert(
  tauriConfig.bundle?.windows?.webviewInstallMode?.type === 'offlineInstaller',
  'Windows bundle must use the offline WebView2 installer',
)
assert(
  tauriConfig.bundle?.windows?.webviewInstallMode?.silent === true,
  'Windows offline WebView2 installer must run silently',
)
assert(
  tauriConfig.bundle?.windows?.wix?.upgradeCode ===
    '818daeb2-ee49-5696-a1db-bee51050939c',
  'Windows WiX upgrade code must stay pinned',
)

includes(
  updaterSource,
  'https://github.com/t41372/PathKeep/releases',
  'updater fallback URL',
)
includes(
  frontendUpdaterSource,
  'https://github.com/t41372/PathKeep/releases',
  'frontend updater fallback URL',
)
includes(
  previewShellCommands,
  'https://github.com/t41372/PathKeep/releases',
  'browser preview updater fallback URL',
)
includes(issueTemplate, 'https://github.com/t41372/PathKeep', 'issue template')

includes(releaseWorkflow, 'unsigned_preview:', 'release workflow')
includes(releaseWorkflow, 'default: true', 'release workflow unsigned_preview')
includes(releaseWorkflow, '- windows', 'release workflow platforms')
includes(
  releaseWorkflow,
  '--no-sign --config src-tauri/ci.unsigned.conf.json',
  'release workflow unsigned build args',
)
includes(
  releaseWorkflow,
  '"bundle": {"createUpdaterArtifacts": False}',
  'release workflow unsigned override',
)

excludes(
  releaseWorkflow,
  'Unsigned preview release builds must not include Windows',
  'release workflow',
)
excludes(releaseWorkflow, 'WINDOWS_CERTIFICATE', 'release workflow')
excludes(releaseWorkflow, 'AZURE_TRUSTED_SIGNING', 'release workflow')
excludes(releaseWorkflow, 'Get-AuthenticodeSignature', 'release workflow')
excludes(issueTemplate, 'BrowserHistoryBackup', 'issue template')
excludes(updaterSource, 'BrowserHistoryBackup', 'updater source')
excludes(
  frontendUpdaterSource,
  'BrowserHistoryBackup',
  'frontend updater source',
)
excludes(previewShellCommands, 'BrowserHistoryBackup', 'preview shell commands')

if (failures.length > 0) {
  console.error('Release configuration drift detected:')
  for (const failure of failures) {
    console.error(`- ${failure}`)
  }
  process.exit(1)
}

console.log('Release configuration OK')
