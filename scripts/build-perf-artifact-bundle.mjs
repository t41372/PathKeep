import { execFileSync } from 'node:child_process'
import { mkdtempSync } from 'node:fs'
import { promises as fs } from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const cwd = process.cwd()
const distDir = path.join(cwd, 'dist')
const manifestPath = path.join(distDir, '.vite', 'manifest.json')

async function ensureFile(filePath) {
  await fs.access(filePath)
  return filePath
}

function unique(values) {
  return [...new Set(values)]
}

async function readJson(filePath) {
  return JSON.parse(await fs.readFile(filePath, 'utf8'))
}

async function fileSize(relativePath) {
  const stats = await fs.stat(path.join(distDir, relativePath))
  return stats.size
}

function entryLabel(manifestKey, manifestEntry) {
  return manifestEntry.name ?? path.basename(path.dirname(manifestKey))
}

function collectEntryAssets(manifest, manifestKey, visited = new Set()) {
  if (visited.has(manifestKey)) {
    return []
  }
  visited.add(manifestKey)

  const entry = manifest[manifestKey]
  if (!entry) {
    return []
  }

  const assets = []
  if (entry.file) {
    assets.push(entry.file)
  }
  if (entry.css) {
    assets.push(...entry.css)
  }
  for (const importKey of entry.imports ?? []) {
    assets.push(...collectEntryAssets(manifest, importKey, visited))
  }
  return unique(assets)
}

async function totalBytes(relativePaths) {
  const sizes = await Promise.all(
    relativePaths.map((relativePath) => fileSize(relativePath)),
  )
  return sizes.reduce((sum, size) => sum + size, 0)
}

function routeBreakdownMarkdown(rows) {
  return [
    '# Route Chunk Breakdown',
    '',
    ...rows.map(
      (row) => `## ${row.route}

- First-load bytes: \`${row.firstLoadBytes}\`
- Route-only bytes: \`${row.routeOnlyBytes}\`
- Entry asset: \`${row.entryAsset}\`
- Shared additions: ${
        row.sharedAdditions.length > 0
          ? row.sharedAdditions.map((asset) => `\`${asset}\``).join(', ')
          : 'None'
      }
`,
    ),
  ].join('\n')
}

function syntheticQueryPlan() {
  const tempRoot = mkdtempSync(path.join(os.tmpdir(), 'pathkeep-perf-'))
  const databasePath = path.join(tempRoot, 'synthetic-shell-scaling.sqlite')
  const sql = `
CREATE TABLE visits (
  id INTEGER PRIMARY KEY,
  url_id INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  reverted_at TEXT
);
CREATE TABLE urls (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT
);
CREATE TABLE source_profiles (
  id INTEGER PRIMARY KEY,
  profile_key TEXT NOT NULL
);
CREATE VIRTUAL TABLE history_search USING fts5(url, title);
INSERT INTO source_profiles (id, profile_key) VALUES (1, 'chrome:Default');
INSERT INTO urls (id, url, title) VALUES (1, 'https://example.com/archive', 'Archive example');
INSERT INTO visits (id, url_id, source_profile_id, reverted_at) VALUES (1, 1, 1, NULL);
INSERT INTO history_search (rowid, url, title) VALUES (1, 'https://example.com/archive', 'Archive example');
EXPLAIN QUERY PLAN
SELECT visits.id
FROM visits
JOIN urls ON urls.id = visits.url_id
JOIN source_profiles ON source_profiles.id = visits.source_profile_id
JOIN history_search ON history_search.rowid = urls.id
WHERE visits.reverted_at IS NULL
  AND history_search MATCH '"example"*';
`

  return execFileSync('sqlite3', [databasePath], {
    encoding: 'utf8',
    input: sql,
  }).trim()
}

async function gitMetadata() {
  const short = execFileSync('git', ['rev-parse', '--short', 'HEAD'], {
    cwd,
    encoding: 'utf8',
  }).trim()
  const full = execFileSync('git', ['rev-parse', 'HEAD'], {
    cwd,
    encoding: 'utf8',
  }).trim()
  return { short, full }
}

async function buildBundle() {
  await ensureFile(manifestPath)
  const manifest = await readJson(manifestPath)
  const generatedAt = new Date().toISOString()
  const artifactDate = generatedAt.slice(0, 10)
  const artifactDir = path.join(
    cwd,
    'artifacts',
    'perf',
    `${artifactDate}-large-archive-shell-scaling`,
  )
  await fs.mkdir(artifactDir, { recursive: true })

  const { short, full } = await gitMetadata()
  const baseAssets = collectEntryAssets(manifest, 'index.html')
  const baseShellBytes = await totalBytes(baseAssets)
  const baseShellSet = new Set(baseAssets)

  const routeBreakdown = []
  for (const [manifestKey, manifestEntry] of Object.entries(manifest)) {
    if (
      !manifestEntry.isDynamicEntry ||
      !manifestKey.startsWith('src/pages/')
    ) {
      continue
    }

    const routeAssets = collectEntryAssets(manifest, manifestKey)
    const firstLoadAssets = unique([...baseAssets, ...routeAssets])
    const sharedAdditions = routeAssets.filter(
      (asset) => !baseShellSet.has(asset),
    )
    routeBreakdown.push({
      route: entryLabel(manifestKey, manifestEntry),
      manifestKey,
      entryAsset: manifestEntry.file,
      firstLoadAssets,
      sharedAdditions,
      firstLoadBytes: await totalBytes(firstLoadAssets),
      routeOnlyBytes: await totalBytes(sharedAdditions),
    })
  }

  routeBreakdown.sort(
    (left, right) =>
      right.firstLoadBytes - left.firstLoadBytes ||
      left.route.localeCompare(right.route),
  )

  const largestRoute = routeBreakdown[0]
  const queryPlan = syntheticQueryPlan()

  const payloadSummary = {
    generatedAt,
    commit: {
      short,
      full,
    },
    buildSource: {
      manifestPath: path.relative(cwd, manifestPath),
      distDir: path.relative(cwd, distDir),
    },
    baseShell: {
      approxBytes: baseShellBytes,
      assets: baseAssets,
    },
    largestFirstRoute: largestRoute
      ? {
          route: largestRoute.route,
          approxBytes: largestRoute.firstLoadBytes,
          routeOnlyBytes: largestRoute.routeOnlyBytes,
        }
      : null,
    routes: routeBreakdown.map((route) => ({
      route: route.route,
      manifestKey: route.manifestKey,
      entryAsset: route.entryAsset,
      approxFirstLoadBytes: route.firstLoadBytes,
      routeOnlyBytes: route.routeOnlyBytes,
      routeOnlyAssets: route.sharedAdditions,
    })),
    syntheticQueryPlanHasVirtualTableIndex: queryPlan.includes(
      'VIRTUAL TABLE INDEX',
    ),
  }

  const context = `# Shell Scaling Context

- Generated at: ${generatedAt}
- Commit: \`${short}\` (${full})
- Artifact type: shell-scaling synthetic bundle
- Build source: \`${path.relative(cwd, manifestPath)}\`
- Base shell approx bytes: \`${baseShellBytes}\`
- Largest first-route approx bytes: \`${largestRoute?.firstLoadBytes ?? 0}\`${largestRoute ? ` (${largestRoute.route})` : ''}
`

  const notes = `# Notes

- This bundle is intentionally limited to shell payload sizing and a synthetic SQLite FTS query plan.
- \`webview-trace.json\` and \`rust-sample.txt\` are placeholders so the bundle layout stays stable until a real large-profile replay is captured.
- A real \`WORK-M4-J\` signoff still requires a true large-profile replay with webview trace and Rust CPU sampling.
`

  const placeholderTrace = JSON.stringify(
    {
      captured: false,
      kind: 'placeholder',
      reason:
        'Synthetic shell-scaling bundle only. Capture a real webview trace during a large-profile replay for final WORK-M4-J signoff.',
      generatedAt,
    },
    null,
    2,
  )

  const placeholderSample = `placeholder: no rust sample captured\nreason: synthetic shell-scaling bundle only\ncaptured_at: ${generatedAt}\n`

  await Promise.all([
    fs.writeFile(path.join(artifactDir, 'context.md'), context),
    fs.writeFile(
      path.join(artifactDir, 'shell-payload-summary.json'),
      `${JSON.stringify(payloadSummary, null, 2)}\n`,
    ),
    fs.writeFile(
      path.join(artifactDir, 'route-chunk-breakdown.md'),
      `${routeBreakdownMarkdown(routeBreakdown)}\n`,
    ),
    fs.writeFile(
      path.join(artifactDir, 'sqlite-query-plan.txt'),
      `${queryPlan}\n`,
    ),
    fs.writeFile(path.join(artifactDir, 'notes.md'), notes),
    fs.writeFile(
      path.join(artifactDir, 'webview-trace.json'),
      `${placeholderTrace}\n`,
    ),
    fs.writeFile(path.join(artifactDir, 'rust-sample.txt'), placeholderSample),
  ])

  console.log(
    JSON.stringify(
      {
        artifactDir: path.relative(cwd, artifactDir),
        baseShellBytes,
        largestRoute: largestRoute
          ? {
              route: largestRoute.route,
              approxBytes: largestRoute.firstLoadBytes,
            }
          : null,
        syntheticQueryPlanHasVirtualTableIndex:
          payloadSummary.syntheticQueryPlanHasVirtualTableIndex,
      },
      null,
      2,
    ),
  )
}

buildBundle().catch((error) => {
  console.error(error instanceof Error ? error.message : error)
  process.exitCode = 1
})
