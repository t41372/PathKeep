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
    'RUSTSEC-2024-0429',
    "glib is only pulled transitively through Tauri's Linux GTK3 stack, and the affected VariantStrIter API is not part of our app code path.",
  ],
  [
    'RUSTSEC-2024-0436',
    'paste is a compile-time proc macro, pulled only through tokenizers (macro_rules_attribute), with no maintained drop-in available upstream yet.',
  ],
  [
    'RUSTSEC-2024-0370',
    'proc-macro-error is only pulled transitively by GTK3 proc-macro dependencies in the current Tauri Linux stack.',
  ],
  [
    'RUSTSEC-2025-0057',
    'fxhash is only pulled through selectors 0.31 by scraper 0.24; scraper 0.29 drops it (WORK-SCRAPER-LATEST).',
  ],
  [
    'RUSTSEC-2026-0459',
    'SQLite before 3.54 can return stale memory when it opens a crafted database with a -wal of another page size. PathKeep builds SQLCipher (bundled-sqlcipher, for the v0.4.0 archive converter), and the patched libsqlite3-sys 0.39 builds it from sqlcipher-src 419.0.6 on SQLite 3.53.4, which the advisory says is still affected; upgrading would only hide it, and add a new dependency. PathKeep opens only the browser files of this computer and files the person picks to import. Remove this entry once libsqlite3-sys bundles SQLCipher on SQLite 3.54 (WORK-SQLCIPHER-SQLITE-354).',
  ],
  ...['RUSTSEC-2026-0370', 'RUSTSEC-2026-0371', 'RUSTSEC-2026-0372'].map(
    (id) => [
      id,
      'zvariant, zvariant_derive and zbus_names were folded into zbus 6, not abandoned. They are only pulled through zbus 5 by tauri-plugin-single-instance 2.4 on Linux, which has no zbus 6 release yet.',
    ],
  ),
])

const allowedNonAdvisoryWarnings = new Map([])

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
