// @vitest-environment node
/** Proves the artifact gate rejects development/mixed packages using compiled probes.
 * Does not claim these tiny fixture bundles are the native desktop distribution. */
import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
  copyFileSync,
  chmodSync,
  existsSync,
} from 'node:fs'
import path from 'node:path'
import { tmpdir } from 'node:os'
// The scripts under test are plain ESM without declaration files, so their shapes are stated here.
interface IdentityReport {
  classification: string
}
interface SidecarModule {
  requireDistributionPackaging: (environment: Record<string, string>) => void
}
interface BuildModule {
  distributionBuildArguments: (input: string[]) => string[]
  verifyBuildOutput: (bundleRoot: string) => string[]
}
interface ArtifactModule {
  inspectBinaryPair: (desktop: string, worker: string) => IdentityReport
  verifyBundle: (bundle: string) => IdentityReport
  checkPackages: (args: string[]) => IdentityReport[]
}

const { requireDistributionPackaging } =
  // @ts-expect-error plain ESM script under test does not ship a declaration file
  (await import('./prepare-worker-sidecar.mjs')) as SidecarModule
const { distributionBuildArguments, verifyBuildOutput } =
  // @ts-expect-error plain ESM script under test does not ship a declaration file
  (await import('./build-distribution.mjs')) as BuildModule
const { inspectBinaryPair, verifyBundle, checkPackages } =
  // @ts-expect-error plain ESM script under test does not ship a declaration file
  (await import('./verify-distribution-artifacts.mjs')) as ArtifactModule

const base = tmpdir()
const library = path.resolve('src-tauri/crates/vault-build/src/lib.rs')

describe('compiled artifact classification', () => {
  it('checks both packaged executables rather than build environment or source flags', () => {
    mkdirSync(base, { recursive: true })
    const dir = mkdtempSync(path.join(base, 'artifact-'))
    try {
      const compile = (
        name: string,
        designation: string | undefined,
        debug: boolean,
        kind: string,
      ) => {
        const source = path.join(dir, `${name}.rs`)
        const binary = path.join(
          dir,
          name + (process.platform === 'win32' ? '.exe' : ''),
        )
        writeFileSync(
          source,
          `#[path = ${JSON.stringify(library)}] mod identity;\nfn main() { assert!(identity::print_if_requested(identity::BinaryKind::${kind})); }\n`,
        )
        const env: NodeJS.ProcessEnv = { ...process.env, TMPDIR: dir }
        delete env.PATHKEEP_DISTRIBUTION
        if (designation) env.PATHKEEP_DISTRIBUTION = designation
        const result = spawnSync(
          'rustc',
          [
            '--edition=2024',
            '-C',
            `debug-assertions=${debug ? 'yes' : 'no'}`,
            '-C',
            'opt-level=0',
            source,
            '-o',
            binary,
          ],
          { env, encoding: 'utf8' },
        )
        expect(result.status, result.stderr).toBe(0)
        return binary
      }
      const desktop = compile('desktop', '1', false, 'Desktop')
      const worker = compile('worker', '1', false, 'Worker')
      const local = compile('local', undefined, false, 'Desktop')
      const debug = compile('debug', '1', true, 'Worker')
      expect(() => {
        inspectBinaryPair(local, worker)
      }).toThrow(/development/)
      expect(() => {
        inspectBinaryPair(desktop, debug)
      }).toThrow(/development/)
      expect(inspectBinaryPair(desktop, worker).classification).toBe(
        'distribution',
      )
      // The extracted payload paths are supplied by the package reader, not source/target guesses.
      const uninspectable = path.join(
        dir,
        'old-binary' + (process.platform === 'win32' ? '.exe' : ''),
      )
      const touched = path.join(dir, 'must-not-be-created')
      const oldSource = path.join(dir, 'old.rs')
      writeFileSync(
        oldSource,
        `fn main() { std::fs::write(${JSON.stringify(touched)}, b"ran").unwrap(); }`,
      )
      expect(
        spawnSync('rustc', [oldSource, '-o', uninspectable], {
          env: { ...process.env, TMPDIR: dir },
        }).status,
      ).toBe(0)
      expect(() => {
        inspectBinaryPair(uninspectable, worker)
      }).toThrow(/does not support --build-identity/)
      expect(existsSync(touched)).toBe(false)
      const payload = path.join(dir, 'payload')
      mkdirSync(path.join(payload, 'usr/bin'), { recursive: true })
      copyFileSync(desktop, path.join(payload, 'usr/bin/pathkeep-desktop'))
      copyFileSync(worker, path.join(payload, 'usr/bin/pathkeep-worker'))
      chmodSync(path.join(payload, 'usr/bin/pathkeep-desktop'), 0o755)
      chmodSync(path.join(payload, 'usr/bin/pathkeep-worker'), 0o755)
      const bundle = path.join(dir, 'candidate.tar.gz')
      expect(
        spawnSync('tar', ['-czf', bundle, '-C', payload, '.']).status,
      ).toBe(0)
      expect(verifyBundle(bundle).classification).toBe('distribution')
      const appRoot = path.join(dir, 'macos/PathKeep.app')
      mkdirSync(path.join(appRoot, 'Contents/MacOS'), { recursive: true })
      copyFileSync(
        desktop,
        path.join(appRoot, 'Contents/MacOS/pathkeep-desktop'),
      )
      copyFileSync(worker, path.join(appRoot, 'Contents/MacOS/pathkeep-worker'))
      writeFileSync(
        path.join(appRoot, 'Contents/Info.plist'),
        '<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>com.yi-ting.pathkeep</string></dict></plist>',
      )
      expect(verifyBuildOutput(path.dirname(appRoot))).toEqual([appRoot])
      expect(existsSync(appRoot + '.tar.gz')).toBe(false)
      // A bundler-produced updater archive is checked as well as the app directory.
      expect(
        spawnSync('tar', [
          '-czf',
          appRoot + '.tar.gz',
          '-C',
          path.dirname(appRoot),
          path.basename(appRoot),
        ]).status,
      ).toBe(0)
      expect(verifyBuildOutput(path.dirname(appRoot))).toEqual(
        expect.arrayContaining([appRoot, appRoot + '.tar.gz']),
      )

      copyFileSync(local, path.join(appRoot, 'Contents/MacOS/pathkeep-desktop'))
      expect(() => {
        verifyBuildOutput(path.dirname(appRoot))
      }).toThrow(/development/)

      copyFileSync(local, path.join(payload, 'usr/bin/pathkeep-desktop'))
      expect(
        spawnSync('tar', ['-czf', bundle, '-C', payload, '.']).status,
      ).toBe(0)
      expect(() => {
        verifyBundle(bundle)
      }).toThrow(/development/)
    } finally {
      rmSync(dir, { recursive: true, force: true })
    }
  }, 30_000)

  it('fails closed in an unamended Release workflow but leaves local preparation alone', () => {
    expect(() => {
      requireDistributionPackaging({
        GITHUB_ACTIONS: 'true',
        GITHUB_WORKFLOW: 'Release',
      })
    }).toThrow(/missing PATHKEEP_DISTRIBUTION/)
    expect(() => {
      requireDistributionPackaging({
        GITHUB_ACTIONS: 'true',
        GITHUB_WORKFLOW: 'Release',
        PATHKEEP_DISTRIBUTION: '1',
      })
    }).not.toThrow()
    expect(() => {
      requireDistributionPackaging({})
    }).not.toThrow()
  })

  it('requires final packages and keeps normal builds on the development installer identity', () => {
    expect(() => {
      checkPackages([])
    }).toThrow(/No artifact means no release approval/)
    expect(() => {
      distributionBuildArguments(['build', '--debug'])
    }).toThrow(/non-debug/)
    expect(() => {
      distributionBuildArguments(['build', '--no-bundle'])
    }).toThrow(/final bundles/)
    expect(
      distributionBuildArguments(['build', '--target', 'aarch64-apple-darwin']),
    ).toContain(path.resolve('src-tauri/tauri.distribution.conf.json'))
    const dev = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'))
    const production = JSON.parse(
      readFileSync('src-tauri/tauri.distribution.conf.json', 'utf8'),
    )
    expect(dev.identifier).toBe('com.yi-ting.pathkeep.dev')
    expect(dev.productName).toBe('PathKeep Dev')
    expect(dev.bundle.windows.wix.upgradeCode).toBe(
      '37e94b55-0506-58ec-9137-e48b4c732ae8',
    )
    expect(production.identifier).toBe('com.yi-ting.pathkeep')
    expect(production.productName).toBe('PathKeep')
    expect(production.bundle.windows.wix.upgradeCode).toBe(
      '818daeb2-ee49-5696-a1db-bee51050939c',
    )
  })

  it('keeps explicit fixture roots in E2E and demo launchers', () => {
    for (const file of [
      'playwright.config.ts',
      'playwright.backup-integrity.config.ts',
      'scripts/dev-demo.mjs',
    ]) {
      expect(readFileSync(file, 'utf8')).toContain('CHB_PROJECT_ROOT:')
    }
  })

  it('keeps the released executable names in the base config without overlay overrides', () => {
    const baseConfig = JSON.parse(
      readFileSync('src-tauri/tauri.conf.json', 'utf8'),
    )
    const release = JSON.parse(
      readFileSync('src-tauri/tauri.distribution.conf.json', 'utf8'),
    )
    expect(baseConfig.mainBinaryName).toBeUndefined()
    expect(baseConfig.bundle.externalBin).toEqual(['binaries/pathkeep-worker'])
    expect(release.mainBinaryName).toBeUndefined()
    expect(release.bundle.externalBin).toBeUndefined()
  })
})
