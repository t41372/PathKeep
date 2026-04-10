import { promises as fs } from 'node:fs'
import path from 'node:path'

const cwd = process.cwd()
const distDir = path.join(cwd, 'dist')
const manifestPath = path.join(distDir, '.vite', 'manifest.json')
const releaseDownloadDir = path.join(cwd, 'dist', 'release')
const bundleDir = path.join(cwd, 'src-tauri', 'target', 'release', 'bundle')
const generatedAt = new Date().toISOString()
const artifactDir = path.join(
  cwd,
  'artifacts',
  'release',
  `${generatedAt.slice(0, 10)}-size-audit`,
)

async function exists(target) {
  try {
    await fs.access(target)
    return true
  } catch {
    return false
  }
}

async function fileSize(target) {
  const stats = await fs.stat(target)
  return stats.size
}

async function walkFiles(root) {
  const entries = await fs.readdir(root, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const target = path.join(root, entry.name)
    if (entry.isDirectory()) {
      files.push(...(await walkFiles(target)))
    } else if (entry.isFile()) {
      files.push(target)
    }
  }
  return files
}

async function collectWebBreakdown() {
  if (!(await exists(manifestPath))) {
    return {
      totalBytes: 0,
      entries: [],
    }
  }

  const manifest = JSON.parse(await fs.readFile(manifestPath, 'utf8'))
  const entries = []
  for (const [manifestKey, value] of Object.entries(manifest)) {
    if (!value?.file) continue
    const assets = [value.file, ...(value.css ?? [])]
    const sizes = await Promise.all(
      assets.map(async (asset) => ({
        asset,
        sizeBytes: await fileSize(path.join(distDir, asset)),
      })),
    )
    entries.push({
      manifestKey,
      entryAsset: value.file,
      assets: sizes,
      totalBytes: sizes.reduce((sum, asset) => sum + asset.sizeBytes, 0),
    })
  }

  return {
    totalBytes: entries.reduce((sum, entry) => sum + entry.totalBytes, 0),
    entries,
  }
}

async function collectReleaseFiles(root) {
  if (!(await exists(root))) {
    return []
  }

  const files = await walkFiles(root)
  const entries = await Promise.all(
    files.map(async (target) => ({
      relativePath: path.relative(root, target),
      sizeBytes: await fileSize(target),
    })),
  )
  return entries.sort((left, right) =>
    left.relativePath.localeCompare(right.relativePath),
  )
}

function categorizeReleaseAsset(relativePath) {
  const fileName = path.basename(relativePath)
  if (fileName === 'latest.json') {
    return 'updater-manifest'
  }
  if (fileName.endsWith('.sig')) {
    return 'updater-signature'
  }
  if (fileName === 'SHA256SUMS.txt') {
    return 'checksum'
  }
  if (fileName === 'RELEASE-MANIFEST.json') {
    return 'release-manifest'
  }
  return 'release-asset'
}

function categorizeLocalBundleArtifact(relativePath) {
  const normalized = relativePath.toLowerCase()
  if (normalized.endsWith('.sig')) {
    return 'updater-signature'
  }
  if (
    normalized.endsWith('.app.tar.gz') ||
    normalized.endsWith('.tar.gz') ||
    normalized.endsWith('.zip')
  ) {
    return 'updater-bundle'
  }
  if (
    normalized.endsWith('.dmg') ||
    normalized.endsWith('.msi') ||
    normalized.endsWith('.exe') ||
    normalized.endsWith('.appimage') ||
    normalized.endsWith('.deb') ||
    normalized.endsWith('.rpm')
  ) {
    return 'installer-bundle'
  }
  return 'support-file'
}

function summarizeByCategory(entries) {
  return Object.entries(
    entries.reduce((summary, entry) => {
      const current = summary[entry.category] ?? { count: 0, totalBytes: 0 }
      current.count += 1
      current.totalBytes += entry.sizeBytes
      summary[entry.category] = current
      return summary
    }, {}),
  )
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([category, totals]) => ({
      category,
      ...totals,
    }))
}

const web = await collectWebBreakdown()
const releaseFiles = (await collectReleaseFiles(releaseDownloadDir)).map(
  (file) => ({
    ...file,
    category: categorizeReleaseAsset(file.relativePath),
  }),
)
const localBundles = (await collectReleaseFiles(bundleDir)).map((file) => ({
  ...file,
  category: categorizeLocalBundleArtifact(file.relativePath),
}))

await fs.mkdir(artifactDir, { recursive: true })

const summary = {
  generatedAt,
  web,
  releaseAssets: releaseFiles,
  localBundleArtifacts: localBundles,
  releaseAssetCategories: summarizeByCategory(releaseFiles),
  localBundleCategories: summarizeByCategory(localBundles),
}

const summaryMarkdown = [
  '# Release Size Audit',
  '',
  `- Generated at: ${generatedAt}`,
  `- Web total bytes: ${web.totalBytes}`,
  `- Downloaded release assets: ${releaseFiles.length}`,
  `- Local bundle artifacts: ${localBundles.length}`,
  '',
  '## Web Entries',
  ...web.entries.map(
    (entry) =>
      `- ${entry.manifestKey}: ${entry.totalBytes} bytes (${entry.assets.map((asset) => `${asset.asset}=${asset.sizeBytes}`).join(', ')})`,
  ),
  '',
  '## Release Asset Categories',
  ...(summary.releaseAssetCategories.length > 0
    ? summary.releaseAssetCategories.map(
        (entry) =>
          `- ${entry.category}: ${entry.count} files, ${entry.totalBytes} bytes`,
      )
    : ['- None found in dist/release']),
  '',
  '## Release Assets',
  ...(releaseFiles.length > 0
    ? releaseFiles.map(
        (file) =>
          `- [${file.category}] ${file.relativePath}: ${file.sizeBytes} bytes`,
      )
    : ['- None found in dist/release']),
  '',
  '## Local Bundle Categories',
  ...(summary.localBundleCategories.length > 0
    ? summary.localBundleCategories.map(
        (entry) =>
          `- ${entry.category}: ${entry.count} files, ${entry.totalBytes} bytes`,
      )
    : ['- None found in src-tauri/target/release/bundle']),
  '',
  '## Local Bundle Artifacts',
  ...(localBundles.length > 0
    ? localBundles.map(
        (file) =>
          `- [${file.category}] ${file.relativePath}: ${file.sizeBytes} bytes`,
      )
    : ['- None found in src-tauri/target/release/bundle']),
].join('\n')

await fs.writeFile(
  path.join(artifactDir, 'size-attribution.json'),
  `${JSON.stringify(summary, null, 2)}\n`,
)
await fs.writeFile(path.join(artifactDir, 'summary.md'), `${summaryMarkdown}\n`)

console.log(`Release size audit written to ${path.relative(cwd, artifactDir)}.`)
