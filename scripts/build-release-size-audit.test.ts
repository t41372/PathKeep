import { afterEach, describe, expect, test } from 'vitest'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'

interface ReleaseSizeAuditModule {
  collectWebBreakdown(input: {
    distDir: string
    manifestPath: string
  }): Promise<{
    totalBytes: number
    entries: Array<{
      manifestKey: string
      entryAsset: string | null
      assets: Array<{ asset: string; sizeBytes: number }>
      totalBytes: number
    }>
    uniqueAssets: Array<{ asset: string; sizeBytes: number }>
  }>
  generateReleaseSizeAudit(input: {
    cwd: string
    generatedAt: string
  }): Promise<{
    artifactDir: string
  }>
}

async function makeFixtureRoot() {
  return await mkdtemp(path.join(os.tmpdir(), 'pathkeep-size-audit-'))
}

async function loadSizeAuditModule(): Promise<ReleaseSizeAuditModule> {
  // @ts-expect-error plain ESM script under test does not ship a declaration file
  return (await import('./build-release-size-audit.mjs')) as ReleaseSizeAuditModule
}

async function seedWebManifest(root: string) {
  const distDir = path.join(root, 'dist')
  await mkdir(path.join(distDir, '.vite'), { recursive: true })
  await writeFile(path.join(distDir, 'assets', 'main.js'), 'main')
  await writeFile(path.join(distDir, 'assets', 'secondary.js'), 'secondary')
  await writeFile(path.join(distDir, 'assets', 'shared.js'), 'shared')
  await writeFile(path.join(distDir, 'assets', 'main.css'), 'styles')

  await writeFile(
    path.join(distDir, '.vite', 'manifest.json'),
    JSON.stringify({
      'src/main.tsx': {
        file: 'assets/main.js',
        css: ['assets/main.css'],
        imports: ['shared-chunk'],
        isEntry: true,
      },
      'src/secondary.tsx': {
        file: 'assets/secondary.js',
        css: ['assets/main.css'],
        imports: ['shared-chunk'],
        isEntry: true,
      },
      'shared-chunk': {
        file: 'assets/shared.js',
      },
    }),
  )
}

const roots: string[] = []

afterEach(async () => {
  await Promise.all(
    roots.splice(0).map((root) => rm(root, { recursive: true, force: true })),
  )
})

describe('build-release-size-audit', () => {
  test('dedupes shared manifest assets while still following imports', async () => {
    const root = await makeFixtureRoot()
    roots.push(root)
    await mkdir(path.join(root, 'dist', 'assets'), { recursive: true })
    await seedWebManifest(root)

    const sizeAudit = await loadSizeAuditModule()
    const web = await sizeAudit.collectWebBreakdown({
      distDir: path.join(root, 'dist'),
      manifestPath: path.join(root, 'dist', '.vite', 'manifest.json'),
    })

    expect(web.totalBytes).toBe(
      'main'.length + 'secondary'.length + 'shared'.length + 'styles'.length,
    )
    expect(web.uniqueAssets.map((asset) => asset.asset)).toEqual([
      'assets/main.css',
      'assets/main.js',
      'assets/secondary.js',
      'assets/shared.js',
    ])
    expect(web.entries).toHaveLength(2)
    expect(
      web.entries[0].assets.some((asset) => asset.asset === 'assets/shared.js'),
    ).toBe(true)
  })

  test('writes same-day audits into distinct timestamped directories', async () => {
    const root = await makeFixtureRoot()
    roots.push(root)
    await mkdir(path.join(root, 'dist', 'assets'), { recursive: true })
    await seedWebManifest(root)

    const sizeAudit = await loadSizeAuditModule()
    const first = await sizeAudit.generateReleaseSizeAudit({
      cwd: root,
      generatedAt: '2026-04-11T05:09:56.896Z',
    })
    const second = await sizeAudit.generateReleaseSizeAudit({
      cwd: root,
      generatedAt: '2026-04-11T08:10:11.111Z',
    })

    expect(first.artifactDir).not.toBe(second.artifactDir)
    expect(path.basename(first.artifactDir)).toContain(
      '2026-04-11T05-09-56-896Z',
    )
    expect(path.basename(second.artifactDir)).toContain(
      '2026-04-11T08-10-11-111Z',
    )
  })
})
