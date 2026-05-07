/**
 * Verifies release configuration that must not drift silently.
 *
 * ## Responsibilities
 * - Keep release URLs pointed at the PathKeep repository.
 * - Keep the Windows release path unsigned and buildable without Windows code-signing secrets.
 * - Keep Windows installers small while still downloading WebView2 when it is missing.
 * - Keep GitHub-hosted workflows off action versions that still run on deprecated Node.js runtimes.
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
import { readdirSync, readFileSync } from 'node:fs'

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
const workflowSources = Object.fromEntries(
  readdirSync('.github/workflows')
    .filter((file) => file.endsWith('.yml') || file.endsWith('.yaml'))
    .map((file) => [file, readText(`.github/workflows/${file}`)]),
)
const allWorkflowText = Object.values(workflowSources).join('\n')
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
  tauriConfig.bundle?.windows?.webviewInstallMode?.type ===
    'downloadBootstrapper',
  'Windows bundle must use the small WebView2 download bootstrapper',
)
assert(
  tauriConfig.bundle?.windows?.webviewInstallMode?.silent === true,
  'Windows WebView2 bootstrapper must run silently',
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
includes(releaseWorkflow, 'taiki-e/install-action@v2', 'release workflow')
includes(releaseWorkflow, 'tool: protoc', 'release workflow')

for (const deprecatedAction of [
  'actions/checkout@v4',
  'actions/setup-node@v4',
  'actions/upload-artifact@v4',
  'actions/cache@v4',
  'arduino/setup-protoc@v3',
]) {
  excludes(allWorkflowText, deprecatedAction, 'GitHub workflows')
}

for (const upgradedAction of [
  'actions/checkout@v6',
  'actions/setup-node@v6',
  'actions/upload-artifact@v7',
  'actions/cache@v5',
]) {
  includes(allWorkflowText, upgradedAction, 'GitHub workflows')
}

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
