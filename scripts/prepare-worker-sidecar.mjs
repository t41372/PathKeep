/**
 * Builds the dedicated worker for the exact Tauri target and stages it under
 * the target-triple filename required by `bundle.externalBin`.
 *
 * Uses only Node.js and Cargo. Tauri supplies `TAURI_ENV_TARGET_TRIPLE` and
 * `TAURI_ENV_DEBUG` to build hooks, while direct invocations fall back to the
 * installed Rust toolchain's host tuple and a release build.
 */
import { spawnSync } from 'node:child_process'
import { chmodSync, copyFileSync, mkdirSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptPath = fileURLToPath(import.meta.url)
const projectRoot = path.resolve(path.dirname(scriptPath), '..')

export function parsePrepareArguments(cliArguments) {
  const parsed = { profile: undefined, target: undefined }
  for (let index = 0; index < cliArguments.length; index += 1) {
    const argument = cliArguments[index]
    if (argument !== '--profile' && argument !== '--target') {
      throw new Error(`Unknown prepare-worker-sidecar argument: ${argument}`)
    }
    const value = cliArguments[index + 1]
    if (!value) {
      throw new Error(`${argument} requires a value`)
    }
    if (argument === '--profile') parsed.profile = value
    if (argument === '--target') parsed.target = value
    index += 1
  }
  if (
    parsed.profile !== undefined &&
    parsed.profile !== 'debug' &&
    parsed.profile !== 'release'
  ) {
    throw new Error(`Unsupported sidecar profile: ${parsed.profile}`)
  }
  return parsed
}

export function selectBuildProfile(explicitProfile, tauriDebug) {
  if (explicitProfile) return explicitProfile
  return tauriDebug === 'true' ? 'debug' : 'release'
}

export function selectTargetTriple(explicitTarget, tauriTarget, hostTarget) {
  const target = explicitTarget ?? tauriTarget ?? hostTarget
  if (!target || !/^[A-Za-z0-9_.-]+$/.test(target)) {
    throw new Error(`Invalid Rust target triple: ${target ?? '<empty>'}`)
  }
  if (target === 'universal-apple-darwin') {
    throw new Error(
      'universal-apple-darwin needs two architecture builds plus lipo; use an architecture-specific release target',
    )
  }
  return target
}

export function sidecarLayout(root, target, profile) {
  const extension = target.includes('-windows-') ? '.exe' : ''
  const fileName = `pathkeep-worker${extension}`
  return {
    source: path.join(root, 'src-tauri', 'target', target, profile, fileName),
    destination: path.join(
      root,
      'src-tauri',
      'binaries',
      `pathkeep-worker-${target}${extension}`,
    ),
  }
}

export function cargoBuildArguments(target, profile) {
  const cargoArguments = [
    'build',
    '--manifest-path',
    'src-tauri/Cargo.toml',
    '--locked',
    '--package',
    'vault-worker',
    '--bin',
    'pathkeep-worker',
    '--target',
    target,
    '--target-dir',
    'src-tauri/target',
  ]
  if (profile === 'release') cargoArguments.push('--release')
  return cargoArguments
}

function commandOutput(command, commandArguments) {
  const result = spawnSync(command, commandArguments, {
    cwd: projectRoot,
    encoding: 'utf8',
  })
  if (result.status !== 0) {
    throw new Error(
      `${command} ${commandArguments.join(' ')} failed: ${result.stderr || result.error || `exit ${result.status}`}`,
    )
  }
  return result.stdout.trim()
}

export function prepareWorkerSidecar({
  cliArguments = process.argv.slice(2),
  environment = process.env,
} = {}) {
  const parsed = parsePrepareArguments(cliArguments)
  const hostTarget =
    parsed.target || environment.TAURI_ENV_TARGET_TRIPLE
      ? undefined
      : commandOutput('rustc', ['--print', 'host-tuple'])
  const target = selectTargetTriple(
    parsed.target,
    environment.TAURI_ENV_TARGET_TRIPLE,
    hostTarget,
  )
  const profile = selectBuildProfile(
    parsed.profile,
    environment.TAURI_ENV_DEBUG,
  )
  const cargoArguments = cargoBuildArguments(target, profile)
  const build = spawnSync('cargo', cargoArguments, {
    cwd: projectRoot,
    env: environment,
    stdio: 'inherit',
  })
  if (build.status !== 0) {
    throw new Error(
      `cargo ${cargoArguments.join(' ')} failed with exit ${build.status ?? 'unknown'}`,
    )
  }

  const layout = sidecarLayout(projectRoot, target, profile)
  mkdirSync(path.dirname(layout.destination), { recursive: true })
  copyFileSync(layout.source, layout.destination)
  if (!target.includes('-windows-')) chmodSync(layout.destination, 0o755)
  const sizeBytes = statSync(layout.destination).size

  console.log(
    `Prepared ${path.relative(projectRoot, layout.destination)} (${sizeBytes} bytes, ${profile})`,
  )
  return { ...layout, profile, sizeBytes, target }
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  try {
    prepareWorkerSidecar()
  } catch (error) {
    console.error(error instanceof Error ? error.message : error)
    process.exit(1)
  }
}
