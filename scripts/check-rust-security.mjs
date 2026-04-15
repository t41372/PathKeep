#!/usr/bin/env bun

import { spawnSync } from 'node:child_process'
import { mkdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const scriptDir = dirname(fileURLToPath(import.meta.url))
const workspaceRoot = resolve(scriptDir, '..')
const tauriRoot = resolve(workspaceRoot, 'src-tauri')

const allowedAdvisories = new Map([
  [
    'RUSTSEC-2024-0411',
    "Tauri's Linux WebKit/GTK3 runtime still depends on gdkwayland-sys until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0412',
    "Tauri's Linux WebKit/GTK3 runtime still depends on gdk until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0413',
    "Tauri's Linux WebKit/GTK3 runtime still depends on atk until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0414',
    "Tauri's Linux WebKit/GTK3 runtime still depends on gdkx11-sys until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0415',
    "Tauri's Linux WebKit/GTK3 runtime still depends on gtk until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0416',
    "Tauri's Linux WebKit/GTK3 runtime still depends on atk-sys until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0417',
    "Tauri's Linux WebKit/GTK3 runtime still depends on gdkx11 until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0418',
    "Tauri's Linux WebKit/GTK3 runtime still depends on gdk-sys until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0419',
    "Tauri's Linux WebKit/GTK3 runtime still depends on gtk3-macros until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0420',
    "Tauri's Linux WebKit/GTK3 runtime still depends on gtk-sys until upstream migrates to GTK4.",
  ],
  [
    'RUSTSEC-2024-0429',
    "glib is only pulled transitively through Tauri's Linux GTK3 stack, and the affected VariantStrIter API is not part of our app code path.",
  ],
  [
    'RUSTSEC-2024-0436',
    'paste is only pulled transitively by upstream stronghold and Turso-backed keyring crates, with no maintained drop-in available without replacing those integrations.',
  ],
  [
    'RUSTSEC-2024-0370',
    'proc-macro-error is only pulled transitively by GTK3 proc-macro dependencies in the current Tauri Linux stack.',
  ],
  [
    'RUSTSEC-2025-0057',
    'fxhash is only pulled transitively by Tauri/urlpattern internals; upstream has not migrated that path yet.',
  ],
  [
    'RUSTSEC-2025-0075',
    'unic-char-range is only pulled transitively by tauri-utils/urlpattern; upstream has not migrated off rust-unic yet.',
  ],
  [
    'RUSTSEC-2025-0080',
    'unic-common is only pulled transitively by tauri-utils/urlpattern; upstream has not migrated off rust-unic yet.',
  ],
  [
    'RUSTSEC-2025-0081',
    'unic-char-property is only pulled transitively by tauri-utils/urlpattern; upstream has not migrated off rust-unic yet.',
  ],
  [
    'RUSTSEC-2025-0098',
    'unic-ucd-version is only pulled transitively by tauri-utils/urlpattern; upstream has not migrated off rust-unic yet.',
  ],
  [
    'RUSTSEC-2025-0100',
    'unic-ucd-ident is only pulled transitively by tauri-utils/urlpattern; upstream has not migrated off rust-unic yet.',
  ],
  [
    'RUSTSEC-2025-0141',
    'bincode 1.x is only pulled transitively by tauri-plugin-stronghold/iota_stronghold; replacing it requires an upstream stronghold migration.',
  ],
  [
    'RUSTSEC-2026-0002',
    "lru 0.12.x is transitively pinned by LanceDB's tantivy stack; PathKeep only reaches tantivy's StoreReader cache methods (get/put/len/peek_lru), not the affected IterMut API.",
  ],
  [
    'RUSTSEC-2026-0097',
    "rand 0.7/0.8/0.9 is currently only present through transitive LanceDB/DataFusion, Stronghold, rig-core/nanoid, and build-time phf tooling; PathKeep's workspace code does not call `rand::rng()` directly, and the reported unsoundness requires a custom logger path that is outside our owned code surface.",
  ],
  [
    'RUSTSEC-2026-0098',
    'rustls-webpki 0.103.10 is only present transitively through reqwest/hyper-rustls in the Tauri updater, rig-core, and LanceDB networking stack; PathKeep does not implement custom URI SAN name-constraint evaluation and is waiting on upstream rustls/request clients to publish the coordinated fix.',
  ],
  [
    'RUSTSEC-2026-0099',
    'rustls-webpki 0.103.10 is only present transitively through reqwest/hyper-rustls in the Tauri updater, rig-core, and LanceDB networking stack; PathKeep does not own certificate wildcard constraint parsing and is waiting on the upstream rustls/request stack to roll the fix.',
  ],
])

const allowedNonAdvisoryWarnings = new Map([
  [
    'yanked:core2@0.4.0',
    'core2 0.4.0 is only pulled transitively through libsodium-sys-stable/libflate in the Stronghold build dependency chain; PathKeep does not depend on it directly and upstream has not published a replacement in that stack yet.',
  ],
])

function fail(message, details = []) {
  console.error(message)
  for (const line of details) {
    console.error(`- ${line}`)
  }
  process.exit(1)
}

function parseAuditJson(stdout, stderr) {
  const candidate = stdout.trim()
  if (candidate.startsWith('{') && candidate.endsWith('}')) {
    return JSON.parse(candidate)
  }

  const combined = `${stdout}\n${stderr}`
  const start = combined.indexOf('{')
  const end = combined.lastIndexOf('}')
  if (start === -1 || end === -1 || end < start) {
    fail('Unable to parse cargo audit JSON output.', [
      'Run `cargo audit --json` in `src-tauri` to inspect the raw output.',
    ])
  }
  return JSON.parse(combined.slice(start, end + 1))
}

function runCargoAudit(extraEnv = {}) {
  return spawnSync('cargo', ['audit', '--json'], {
    cwd: tauriRoot,
    encoding: 'utf8',
    env: {
      ...process.env,
      ...extraEnv,
    },
  })
}

let audit = runCargoAudit()

if (
  audit.stderr.includes(
    'attempted to take an exclusive lock on a read-only path',
  )
) {
  const fallbackCargoHome = resolve(tmpdir(), 'pathkeep-cargo-home')
  mkdirSync(fallbackCargoHome, { recursive: true })
  audit = runCargoAudit({ CARGO_HOME: fallbackCargoHome })
}

if (audit.error) {
  fail('Failed to execute `cargo audit --json`.', [audit.error.message])
}

const auditReport = parseAuditJson(audit.stdout, audit.stderr)
const vulnerabilities = auditReport.vulnerabilities?.list ?? []
const warnings = Object.values(auditReport.warnings ?? {}).flat()
const unexpectedVulnerabilities = []
const encounteredAdvisories = new Map()

for (const entry of vulnerabilities) {
  const summary = `${entry.advisory.id} (${entry.package.name}@${entry.package.version})`
  encounteredAdvisories.set(entry.advisory.id, summary)
  if (!allowedAdvisories.has(entry.advisory.id)) {
    unexpectedVulnerabilities.push(`${summary}: ${entry.advisory.title}`)
  }
}

if (unexpectedVulnerabilities.length > 0) {
  fail(
    'Unexpected RustSec vulnerabilities were detected.',
    unexpectedVulnerabilities,
  )
}

const unexpectedWarnings = []
const encounteredNonAdvisoryWarnings = new Map()

for (const entry of warnings) {
  const packageSummary = `${entry.package.name}@${entry.package.version}`
  if (entry.advisory) {
    const summary = `${entry.advisory.id} (${packageSummary})`
    encounteredAdvisories.set(entry.advisory.id, summary)
    if (!allowedAdvisories.has(entry.advisory.id)) {
      unexpectedWarnings.push(`${summary}: ${entry.advisory.title}`)
    }
    continue
  }

  const nonAdvisoryKey = `${entry.kind}:${packageSummary}`
  encounteredNonAdvisoryWarnings.set(nonAdvisoryKey, entry.kind)
  if (!allowedNonAdvisoryWarnings.has(nonAdvisoryKey)) {
    unexpectedWarnings.push(
      `${nonAdvisoryKey}: cargo audit reported a non-advisory ${entry.kind} warning for ${packageSummary}.`,
    )
  }
}

const staleAdvisoryAllowlist = [...allowedAdvisories.keys()]
  .filter((id) => !encounteredAdvisories.has(id))
  .map((id) => `${id}: ${allowedAdvisories.get(id)}`)

const staleNonAdvisoryAllowlist = [...allowedNonAdvisoryWarnings.keys()]
  .filter((key) => !encounteredNonAdvisoryWarnings.has(key))
  .map((key) => `${key}: ${allowedNonAdvisoryWarnings.get(key)}`)

if (unexpectedWarnings.length > 0) {
  fail('Unexpected RustSec warnings were detected.', unexpectedWarnings)
}

if (staleAdvisoryAllowlist.length > 0 || staleNonAdvisoryAllowlist.length > 0) {
  fail(
    'The Rust supply-chain allowlist contains entries that are no longer present.',
    [...staleAdvisoryAllowlist, ...staleNonAdvisoryAllowlist],
  )
}

if (audit.status !== 0 && encounteredAdvisories.size === 0) {
  fail(
    '`cargo audit --json` exited unsuccessfully without a reportable advisory delta.',
    [`Exit status: ${audit.status ?? 'unknown'}`],
  )
}

console.log('Rust supply-chain audit passed.')
if (audit.status !== 0) {
  console.log(
    `cargo audit exited with status ${audit.status}, but every advisory/warning was explicitly allowlisted.`,
  )
}
console.log(`Allowed RustSec advisories: ${encounteredAdvisories.size}`)
console.log(
  `Allowed non-advisory warnings: ${encounteredNonAdvisoryWarnings.size}`,
)

for (const id of [...encounteredAdvisories.keys()].sort()) {
  console.log(`- ${id}: ${allowedAdvisories.get(id)}`)
}

for (const key of [...encounteredNonAdvisoryWarnings.keys()].sort()) {
  console.log(`- ${key}: ${allowedNonAdvisoryWarnings.get(key)}`)
}
