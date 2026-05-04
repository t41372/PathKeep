#!/usr/bin/env node
/**
 * Project-scoped native dependency helper.
 *
 * Responsibilities:
 * - Bootstrap a pinned vcpkg checkout under var/native-deps.
 * - Install manifest features into a repo-local vcpkg_installed tree.
 * - Print deterministic environment values for future Rust/CMake bridge work.
 *
 * Not responsible for:
 * - Linking native libraries into product code.
 * - Installing global CMake, pkg-config, OpenCC, Homebrew, apt, or winget packages.
 * - Replacing OS SDKs and platform frameworks required by Tauri itself.
 *
 * Dependencies:
 * - Node.js, git, and the platform C/C++ toolchain needed by vcpkg ports.
 *
 * Performance notes:
 * - vcpkg builds can be slow; this helper writes only under ignored var/native-deps
 *   so binary caches can be layered by CI without polluting source control.
 */

import { existsSync, mkdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
)
const nativeRoot = path.join(repoRoot, 'var', 'native-deps')
const vcpkgRoot = path.join(nativeRoot, 'vcpkg')
const installRoot = path.join(nativeRoot, 'vcpkg_installed')
const configurationPath = path.join(repoRoot, 'vcpkg-configuration.json')
const manifestPath = path.join(repoRoot, 'vcpkg.json')
const vcpkgRepository = 'https://github.com/microsoft/vcpkg'

function main() {
  const [command, ...args] = process.argv.slice(2)
  switch (command) {
    case 'doctor':
      doctor()
      break
    case 'bootstrap':
      bootstrap()
      break
    case 'install':
      install(args)
      break
    case 'env':
      printEnvironment()
      break
    default:
      usage()
      process.exitCode = 1
  }
}

function usage() {
  console.log(`Usage:
  node scripts/native-deps.mjs doctor
  node scripts/native-deps.mjs bootstrap
  node scripts/native-deps.mjs install --feature=opencc [--triplet=<triplet>]
  node scripts/native-deps.mjs env`)
}

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, 'utf8'))
}

function expectedBaseline() {
  const configuration = readJson(configurationPath)
  const baseline = configuration?.['default-registry']?.baseline
  if (!baseline || !/^[0-9a-f]{40}$/u.test(baseline)) {
    throw new Error(
      `vcpkg-configuration.json must pin a 40-character default-registry baseline`,
    )
  }
  return baseline
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    env: options.env ?? process.env,
    encoding: 'utf8',
    stdio: options.capture ? 'pipe' : 'inherit',
    shell: process.platform === 'win32',
  })
  if (result.status !== 0) {
    const suffix = options.capture ? `\n${result.stderr}${result.stdout}` : ''
    throw new Error(`${command} ${args.join(' ')} failed${suffix}`)
  }
  return result.stdout?.trim() ?? ''
}

function commandPath(name) {
  const result =
    process.platform === 'win32'
      ? spawnSync('where', [name], { encoding: 'utf8', shell: true })
      : spawnSync('sh', ['-lc', `command -v ${name}`], { encoding: 'utf8' })
  return result.status === 0 ? result.stdout.trim().split(/\r?\n/u)[0] : null
}

function ensurePinnedVcpkgCheckout() {
  const baseline = expectedBaseline()
  mkdirSync(nativeRoot, { recursive: true })
  if (!existsSync(path.join(vcpkgRoot, '.git'))) {
    mkdirSync(vcpkgRoot, { recursive: true })
    run('git', ['init', vcpkgRoot])
    run('git', ['-C', vcpkgRoot, 'remote', 'add', 'origin', vcpkgRepository])
  }
  run('git', ['-C', vcpkgRoot, 'fetch', '--depth', '1', 'origin', baseline])
  run('git', ['-C', vcpkgRoot, 'checkout', '--detach', 'FETCH_HEAD'])
  const actual = run('git', ['-C', vcpkgRoot, 'rev-parse', 'HEAD'], {
    capture: true,
  })
  if (actual !== baseline) {
    throw new Error(
      `vcpkg checkout drifted: expected ${baseline}, got ${actual}`,
    )
  }
}

function vcpkgBinary() {
  return path.join(
    vcpkgRoot,
    process.platform === 'win32' ? 'vcpkg.exe' : 'vcpkg',
  )
}

function bootstrap() {
  ensurePinnedVcpkgCheckout()
  if (existsSync(vcpkgBinary())) {
    console.log(`vcpkg already bootstrapped at ${vcpkgBinary()}`)
    return
  }
  const script =
    process.platform === 'win32'
      ? 'bootstrap-vcpkg.bat'
      : './bootstrap-vcpkg.sh'
  run(script, ['-disableMetrics'], { cwd: vcpkgRoot })
}

function install(args) {
  bootstrap()
  const features = args
    .filter((arg) => arg.startsWith('--feature='))
    .map((arg) => arg.slice('--feature='.length))
    .filter(Boolean)
  const triplet = args
    .find((arg) => arg.startsWith('--triplet='))
    ?.slice('--triplet='.length)
  const manifest = readJson(manifestPath)
  for (const feature of features) {
    if (!manifest.features?.[feature]) {
      throw new Error(`Unknown vcpkg manifest feature: ${feature}`)
    }
  }
  const installArgs = [
    'install',
    `--x-manifest-root=${repoRoot}`,
    `--x-install-root=${installRoot}`,
    '--clean-after-build',
    ...features.map((feature) => `--x-feature=${feature}`),
  ]
  if (triplet) {
    installArgs.push(`--triplet=${triplet}`)
  }
  run(vcpkgBinary(), installArgs, {
    env: {
      ...process.env,
      VCPKG_DISABLE_METRICS: '1',
      VCPKG_ROOT: vcpkgRoot,
      VCPKG_INSTALLED_DIR: installRoot,
    },
  })
}

function doctor() {
  const baseline = expectedBaseline()
  const manifest = readJson(manifestPath)
  const openccFeature = Boolean(manifest.features?.opencc)
  const gitPath = commandPath('git')
  const compilerPath =
    commandPath(process.platform === 'win32' ? 'cl' : 'clang++') ??
    commandPath('c++') ??
    commandPath('g++')
  console.log(`native deps root: ${nativeRoot}`)
  console.log(`vcpkg root: ${vcpkgRoot}`)
  console.log(`vcpkg install root: ${installRoot}`)
  console.log(`vcpkg baseline: ${baseline}`)
  console.log(`vcpkg OpenCC feature declared: ${openccFeature ? 'yes' : 'no'}`)
  console.log(`git: ${gitPath ?? 'not found'}`)
  console.log(`C++ compiler: ${compilerPath ?? 'not found'}`)
  console.log(
    `bootstrapped vcpkg: ${existsSync(vcpkgBinary()) ? vcpkgBinary() : 'not found'}`,
  )
}

function printEnvironment() {
  console.log(`export VCPKG_ROOT=${shellQuote(vcpkgRoot)}`)
  console.log(`export VCPKG_INSTALLED_DIR=${shellQuote(installRoot)}`)
  console.log(
    `export CMAKE_TOOLCHAIN_FILE=${shellQuote(
      path.join(vcpkgRoot, 'scripts', 'buildsystems', 'vcpkg.cmake'),
    )}`,
  )
}

function shellQuote(value) {
  return `'${value.replaceAll("'", "'\\''")}'`
}

main()
