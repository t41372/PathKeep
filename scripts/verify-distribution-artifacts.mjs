/** Responsibilities: reject non-distribution or mismatched executables in final package payloads.
 * Not responsible for: building, signing, publishing, or opening an archive. Uses Node built-ins
 * and the build host's archive tools; an unsupported/unextractable package fails closed. */
import { spawnSync } from 'node:child_process'
import {
  lstatSync,
  readdirSync,
  mkdtempSync,
  mkdirSync,
  rmSync,
  copyFileSync,
  writeFileSync,
  openSync,
  readSync,
  closeSync,
} from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const production = {
  schemaVersion: 1,
  classification: 'distribution',
  debugAssertions: false,
  defaultRootQualifier: 'com',
  defaultRootOrganization: 'yi-ting',
  defaultRootApplication: 'pathkeep',
  bundleIdentifier: 'com.yi-ting.pathkeep',
  keyringService: 'com.yi-ting.pathkeep',
  scheduleLabel: 'com.yi-ting.pathkeep.backup',
  loginItemLabel: 'com.yi-ting.pathkeep.login',
  autostartName: 'PathKeep',
  windowsUpgradeCode: '818daeb2-ee49-5696-a1db-bee51050939c',
}
const desktopNames = new Set([
  'pathkeep-desktop',
  'pathkeep-desktop.exe',
  'PathKeep',
  'PathKeep.exe',
  'pathkeep',
  'pathkeep.exe',
  'PathKeep Dev',
  'PathKeep Dev.exe',
])
const workerNames = new Set(['pathkeep-worker', 'pathkeep-worker.exe'])

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
    timeout: 120_000,
    ...options,
    // A binary with a broken probe must fail before root effects, not use a host default.
    env: { ...process.env, ...options.env, CHB_PROJECT_ROOT: '' },
  })
  if (result.error || result.status !== 0) {
    throw new Error(
      `${program} ${args.join(' ')} failed: ${result.error?.message || result.stderr || `exit ${result.status}`}`,
    )
  }
  return result.stdout
}

function supportsProbe(binary) {
  const needle = Buffer.from('--build-identity')
  const file = openSync(binary, 'r')
  const chunk = Buffer.alloc(64 * 1024)
  let tail = Buffer.alloc(0)
  try {
    for (;;) {
      const size = readSync(file, chunk)
      if (!size) return false
      const bytes = Buffer.concat([tail, chunk.subarray(0, size)])
      if (bytes.includes(needle)) return true
      tail = Buffer.from(
        bytes.subarray(Math.max(0, bytes.length - needle.length)),
      )
    }
  } finally {
    closeSync(file)
  }
}

function probe(binary, kind) {
  if (!supportsProbe(binary))
    throw new Error(
      `${kind} does not support --build-identity; refusing to launch an uninspectable artifact: ${binary}`,
    )
  const raw = run(path.resolve(binary), ['--build-identity'], {
    timeout: 15_000,
  })
  let value
  try {
    value = JSON.parse(raw)
  } catch {
    throw new Error(`${kind} did not return a build-identity report: ${binary}`)
  }
  if (value.binary !== kind)
    throw new Error(`Expected ${kind}, got ${value.binary}: ${binary}`)
  for (const [key, expected] of Object.entries(production)) {
    if (value[key] !== expected) {
      throw new Error(
        `${kind} is not a distribution artifact: ${key}=${JSON.stringify(value[key])}; expected ${JSON.stringify(expected)}. Rebuild with scripts/build-distribution.mjs; do not publish this package.`,
      )
    }
  }
  return value
}

/** Both payload executables must agree; environment flags are never evidence of what was built. */
export function inspectBinaryPair(desktop, worker) {
  const app = probe(desktop, 'desktop')
  const sidecar = probe(worker, 'worker')
  const withoutKind = (value) =>
    Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => key !== 'binary')
        .sort(([a], [b]) => a.localeCompare(b)),
    )
  if (
    JSON.stringify(withoutKind(app)) !== JSON.stringify(withoutKind(sidecar))
  ) {
    throw new Error(
      'Packaged desktop and worker build identities disagree; do not publish.',
    )
  }
  return {
    ...withoutKind(app),
    desktop: path.resolve(desktop),
    worker: path.resolve(worker),
  }
}

function* filesUnder(root) {
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const file = path.join(root, entry.name)
    if (entry.isDirectory()) yield* filesUnder(file)
    else if (entry.isFile()) yield file
  }
}

function inspectPayload(root) {
  const files = [...filesUnder(root)]
  const desktops = files.filter((file) => desktopNames.has(path.basename(file)))
  const workers = files.filter((file) => workerNames.has(path.basename(file)))
  if (desktops.length === 0 && workers.length === 0) {
    const nested = files.filter((file) =>
      /\.(AppImage|msi|exe|tar\.gz)$/.test(file),
    )
    if (nested.length === 1) return verifyBundle(nested[0])
  }
  if (desktops.length !== 1 || workers.length !== 1) {
    throw new Error(
      `Expected one packaged desktop and worker in ${root}; found ${desktops.length} desktop(s), ${workers.length} worker(s). Source/target binaries are not a substitute.`,
    )
  }
  const info = files.find((file) =>
    file.endsWith(`${path.sep}Contents${path.sep}Info.plist`),
  )
  if (info && process.platform === 'darwin') {
    const identifier = run('plutil', [
      '-extract',
      'CFBundleIdentifier',
      'raw',
      '-o',
      '-',
      info,
    ]).trim()
    if (identifier !== production.bundleIdentifier)
      throw new Error(
        `Packaged bundle identifier is ${identifier}, not ${production.bundleIdentifier}`,
      )
  }
  return inspectBinaryPair(desktops[0], workers[0])
}

function unpackWindows(bundle, destination) {
  // 7-Zip is supplied by the Windows release runner, not downloaded by this check.
  run('7z', ['x', '-y', `-o${destination}`, bundle])
  const cabinets = [...filesUnder(destination)].filter((file) =>
    file.toLowerCase().endsWith('.cab'),
  )
  for (const [index, cabinet] of cabinets.entries()) {
    run('7z', [
      'x',
      '-y',
      `-o${path.join(destination, `cab-${index}`)}`,
      cabinet,
    ])
  }
  if (bundle.toLowerCase().endsWith('.msi')) {
    // MSI cabinets may use File-table IDs instead of original filenames. Read the table
    // without installing or running custom actions, then restore the executable leaf names.
    const script = `param([string]$Package)
$installer = New-Object -ComObject WindowsInstaller.Installer
$db = $installer.OpenDatabase($Package, 0)
$view = $db.OpenView('SELECT \`File\`, \`FileName\` FROM \`File\`')
$view.Execute()
$rows = @()
while ($record = $view.Fetch()) { $rows += @{ id = $record.StringData(1); name = ($record.StringData(2) -split '\\|')[-1] } }
ConvertTo-Json -InputObject $rows -Compress`
    const scriptPath = path.join(destination, 'read-msi-files.ps1')
    writeFileSync(scriptPath, script)
    const rows = JSON.parse(
      run('powershell.exe', [
        '-NoProfile',
        '-NonInteractive',
        '-ExecutionPolicy',
        'Bypass',
        '-File',
        scriptPath,
        bundle,
      ]),
    )
    const files = [...filesUnder(destination)]
    for (const row of rows) {
      if (!desktopNames.has(row.name) && !workerNames.has(row.name)) continue
      if (files.some((file) => path.basename(file) === row.name)) continue
      const source = files.find((file) => path.basename(file) === row.id)
      if (!source)
        throw new Error(`MSI payload is missing ${row.name} (${row.id})`)
      const folder = path.join(destination, 'identity-payload')
      mkdirSync(folder, { recursive: true })
      copyFileSync(source, path.join(folder, row.name))
    }
  }
}

/** Inspect an actual final archive/installer, never a guessed Cargo target executable. */
export function verifyBundle(input) {
  const bundle = path.resolve(input)
  if (lstatSync(bundle).isDirectory()) {
    if (!bundle.endsWith('.app'))
      throw new Error(
        'Only a packaged .app directory is accepted; pass the final package file.',
      )
    return inspectPayload(bundle)
  }
  const work = mkdtempSync(path.join(path.dirname(bundle), '.identity-check-'))
  let mounted = false
  try {
    if (/\.(tar\.gz|tgz)$/.test(bundle))
      run('tar', ['-xzf', bundle, '-C', work])
    else if (bundle.endsWith('.dmg')) {
      const mount = path.join(work, 'mount')
      mkdirSync(mount)
      run('hdiutil', [
        'attach',
        '-readonly',
        '-nobrowse',
        '-mountpoint',
        mount,
        bundle,
      ])
      mounted = true
    } else if (bundle.endsWith('.zip')) {
      if (process.platform === 'win32')
        run('7z', ['x', '-y', `-o${work}`, bundle])
      else run('unzip', ['-q', bundle, '-d', work])
    } else if (bundle.endsWith('.deb')) run('dpkg-deb', ['-x', bundle, work])
    else if (bundle.endsWith('.rpm')) {
      run(
        'bash',
        [
          '-o',
          'pipefail',
          '-c',
          'rpm2cpio "$1" | cpio -idm --quiet',
          'extract-rpm',
          bundle,
        ],
        { cwd: work },
      )
    } else if (bundle.endsWith('.AppImage'))
      run(bundle, ['--appimage-extract'], { cwd: work })
    else if (/\.(exe|msi)$/i.test(bundle) && process.platform === 'win32')
      unpackWindows(bundle, work)
    else
      throw new Error(`Unsupported package for identity inspection: ${bundle}`)
    return inspectPayload(work)
  } finally {
    if (mounted) run('hdiutil', ['detach', path.join(work, 'mount')])
    rmSync(work, { recursive: true, force: true })
  }
}

export function checkPackages(args) {
  const packages = []
  for (let index = 0; index < args.length; index += 2) {
    if (args[index] !== '--package' || !args[index + 1])
      throw new Error(
        'Usage: release:check --package <final artifact> [--package <final artifact> ...]',
      )
    packages.push(args[index + 1])
  }
  if (!packages.length)
    throw new Error(
      'release:check requires --package <final artifact>; use release:check:config for source-only checks. No artifact means no release approval.',
    )
  return packages.map((file) => ({
    package: path.resolve(file),
    ...verifyBundle(file),
  }))
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    // Retain the existing source/configuration checks as well as the executable proof.
    run(process.execPath, [path.resolve('scripts/verify-release-config.mjs')])
    console.log(JSON.stringify(checkPackages(process.argv.slice(2)), null, 2))
  } catch (error) {
    console.error(`Release artifact check failed: ${error.message}`)
    process.exitCode = 1
  }
}
