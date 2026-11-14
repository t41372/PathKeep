/** Responsibilities: designate distribution compilation, then inspect final bundles before upload.
 * Not responsible for: publishing, signing credentials, development builds or archive migrations. */
import { spawnSync } from 'node:child_process'
import { existsSync, readdirSync, rmSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { verifyBundle } from './verify-distribution-artifacts.mjs'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

export function distributionBuildArguments(input) {
  const args = input[0] === 'build' ? input.slice(1) : input
  if (args.includes('--debug') || args.includes('--no-bundle'))
    throw new Error('Distribution packaging requires non-debug final bundles.')
  return [
    'build',
    ...args,
    '--config',
    path.join(root, 'src-tauri/tauri.distribution.conf.json'),
  ]
}

export function buildDistribution(input = process.argv.slice(2)) {
  const args = distributionBuildArguments(input)
  const config = spawnSync(
    process.execPath,
    [path.join(root, 'scripts/verify-release-config.mjs')],
    { cwd: root, stdio: 'inherit' },
  )
  if (config.status !== 0)
    throw new Error(
      'Release configuration check failed; no distribution build was started.',
    )
  const targetIndex = args.findIndex(
    (argument) => argument === '--target' || argument === '-t',
  )
  const target = targetIndex < 0 ? undefined : args[targetIndex + 1]
  if (target && !/^[A-Za-z0-9_-]+$/.test(target))
    throw new Error('Invalid distribution target')
  const targetRoot = path.join(root, 'src-tauri/target')
  if (
    process.env.CARGO_TARGET_DIR &&
    path.resolve(process.env.CARGO_TARGET_DIR) !== targetRoot
  )
    throw new Error(
      'Distribution packaging requires the repository target directory for artifact verification.',
    )
  const bundleRoot = path.join(
    targetRoot,
    ...(target ? [target] : []),
    'release/bundle',
  )
  // Only reproducible package outputs: stale packages must not be mistaken for this build.
  rmSync(bundleRoot, { recursive: true, force: true })
  const result = spawnSync('bun', ['x', 'tauri', ...args], {
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, PATHKEEP_DISTRIBUTION: '1' },
  })
  if (result.error || result.status !== 0)
    throw new Error(
      `Distribution build failed: ${result.error?.message || result.status}`,
    )
  return verifyBuildOutput(bundleRoot)
}

/** Check existing bundle output, including app directories, without creating upload artifacts. */
export function verifyBuildOutput(bundleRoot) {
  const packages = []
  function walk(folder) {
    for (const entry of readdirSync(folder, { withFileTypes: true })) {
      const file = path.join(folder, entry.name)
      if (entry.isDirectory()) {
        if (entry.name.endsWith('.app')) packages.push(file)
        else walk(file)
      } else if (/\.(dmg|deb|rpm|AppImage|msi|exe|zip|tar\.gz)$/.test(file))
        packages.push(file)
    }
  }
  if (existsSync(bundleRoot)) walk(bundleRoot)
  if (!packages.length)
    throw new Error(`No final packages found in ${bundleRoot}; do not publish.`)
  for (const file of packages) {
    verifyBundle(file)
    console.log(`Distribution identity verified: ${file}`)
  }
  return packages
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    buildDistribution()
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
