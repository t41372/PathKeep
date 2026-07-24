import { describe, expect, test } from 'vitest'
import path from 'node:path'

interface PrepareWorkerSidecarModule {
  cargoBuildArguments(target: string, profile: string): string[]
  parsePrepareArguments(arguments_: string[]): {
    profile?: string
    target?: string
  }
  selectBuildProfile(
    explicitProfile: string | undefined,
    tauriDebug: string | undefined,
  ): string
  selectTargetTriple(
    explicitTarget: string | undefined,
    tauriTarget: string | undefined,
    hostTarget: string | undefined,
  ): string
  sidecarLayout(
    root: string,
    target: string,
    profile: string,
  ): { source: string; destination: string }
}

async function loadModule(): Promise<PrepareWorkerSidecarModule> {
  // @ts-expect-error plain ESM script under test does not ship a declaration file
  return (await import('../../scripts/prepare-worker-sidecar.mjs')) as PrepareWorkerSidecarModule
}

describe('prepare-worker-sidecar', () => {
  test('uses the Tauri target and debug flag unless explicitly overridden', async () => {
    const prepare = await loadModule()

    expect(
      prepare.selectTargetTriple(
        undefined,
        'aarch64-apple-darwin',
        'x86_64-apple-darwin',
      ),
    ).toBe('aarch64-apple-darwin')
    expect(
      prepare.selectTargetTriple(
        'x86_64-apple-darwin',
        'aarch64-apple-darwin',
        undefined,
      ),
    ).toBe('x86_64-apple-darwin')
    expect(prepare.selectBuildProfile(undefined, 'true')).toBe('debug')
    expect(prepare.selectBuildProfile(undefined, 'false')).toBe('release')
    expect(prepare.selectBuildProfile('debug', 'false')).toBe('debug')
  })

  test('builds and stages the exact target-triple filename Tauri requires', async () => {
    const prepare = await loadModule()
    const root = path.join(path.sep, 'workspace')
    const windows = prepare.sidecarLayout(
      root,
      'x86_64-pc-windows-msvc',
      'release',
    )

    expect(windows.source).toBe(
      path.join(
        root,
        'src-tauri',
        'target',
        'x86_64-pc-windows-msvc',
        'release',
        'pathkeep-worker.exe',
      ),
    )
    expect(windows.destination).toBe(
      path.join(
        root,
        'src-tauri',
        'binaries',
        'pathkeep-worker-x86_64-pc-windows-msvc.exe',
      ),
    )
    expect(
      prepare.cargoBuildArguments('aarch64-apple-darwin', 'release'),
    ).toEqual([
      'build',
      '--manifest-path',
      'src-tauri/Cargo.toml',
      '--locked',
      '--package',
      'vault-worker',
      '--bin',
      'pathkeep-worker',
      '--target',
      'aarch64-apple-darwin',
      '--target-dir',
      'src-tauri/target',
      '--release',
    ])
  })

  test('rejects malformed and unsupported build requests', async () => {
    const prepare = await loadModule()

    expect(() => prepare.parsePrepareArguments(['--profile', 'fast'])).toThrow(
      'Unsupported sidecar profile',
    )
    expect(() =>
      prepare.selectTargetTriple(undefined, 'bad target', undefined),
    ).toThrow('Invalid Rust target triple')
    expect(() =>
      prepare.selectTargetTriple(
        undefined,
        'universal-apple-darwin',
        undefined,
      ),
    ).toThrow('needs two architecture builds plus lipo')
  })
})
